#!/usr/bin/env python3
"""Create immutable, dependency-free PERF-091 microbenchmark observations."""
import argparse
import csv
import datetime as dt
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "experiments/static-payload-costs/main.rs"
RUSTC = Path("/home/bat/projects/rust-actor-runtime-lab/.tools/rustup/toolchains/1.98.1-x86_64-unknown-linux-gnu/bin/rustc")
HERE = Path(__file__).resolve().parent
SAMPLES = 9


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def dump(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def quantile(values, fraction):
    values = sorted(values)
    position = (len(values) - 1) * fraction
    low = int(position)
    high = min(low + 1, len(values) - 1)
    return values[low] + (values[high] - values[low]) * (position - low)


def distribution(values):
    return {
        "samples": values,
        "min": min(values),
        "p25": quantile(values, 0.25),
        "median": statistics.median(values),
        "p75": quantile(values, 0.75),
        "max": max(values),
    }


def parse_rows(directory):
    rows = []
    for sample in range(SAMPLES):
        raw = (directory / f"sample-{sample:02}.stdout").read_text()
        assert "# correctness=passed" in raw
        reader = csv.DictReader(io.StringIO("\n".join(
            line for line in raw.splitlines() if not line.startswith("#")
        )), delimiter="\t")
        sample_rows = []
        for row in reader:
            for key in row.keys() - {"mix", "kind"}:
                row[key] = int(row[key])
            assert row["sample"] == sample
            assert row["views"] == row["fanout"] * row["rounds"]
            assert row["views"] >= 2_000_000 and row["elapsed_ns"] > 0
            sample_rows.append(row)
        assert len(sample_rows) == 63
        assert {r["order"] for r in sample_rows} == set(range(63))
        expected = {
            (size, width, mix, kind)
            for size in (8, 256, 4096)
            for width in (1, 16, 256)
            for mix in ("static", "mixed50", "dynamic")
            for kind in (("arc", "direct", "tagged") if mix == "static" else ("arc", "tagged"))
        }
        assert {(r["bytes"], r["fanout"], r["mix"], r["kind"]) for r in sample_rows} == expected
        for size in (8, 256, 4096):
            for width in (1, 16, 256):
                values = [r for r in sample_rows if r["bytes"] == size and r["fanout"] == width]
                assert len({r["checksum"] for r in values}) == 1
                assert len({r["views"] for r in values}) == 1
        rows.extend(sample_rows)
    return rows


def summarize(rows):
    groups = {}
    for row in rows:
        key = (row["bytes"], row["fanout"], row["mix"], row["kind"])
        groups.setdefault(key, []).append(row)
    result = {"configurations": [], "comparisons": []}
    for key, values in sorted(groups.items()):
        values.sort(key=lambda row: row["sample"])
        assert len(values) == SAMPLES
        size, width, mix, kind = key
        record = {"bytes": size, "fanout": width, "mix": mix, "kind": kind}
        record["ns_per_view"] = distribution([r["elapsed_ns"] / r["views"] for r in values])
        record["elapsed_ns"] = distribution([r["elapsed_ns"] for r in values])
        result["configurations"].append(record)
        if kind == "arc":
            continue
        baseline = sorted(groups[(size, width, mix, "arc")], key=lambda row: row["sample"])
        paired = [100 * (1 - treatment["elapsed_ns"] / base["elapsed_ns"])
                  for treatment, base in zip(values, baseline)]
        enough_time = min(statistics.median(r["elapsed_ns"] for r in values),
                          statistics.median(r["elapsed_ns"] for r in baseline)) >= 5_000_000
        positive = sum(value > 5 for value in paired)
        negative = sum(value < -5 for value in paired)
        median = statistics.median(paired)
        verdict = "inconclusive"
        if enough_time and median > 10 and positive >= 8:
            verdict = "locally_consistent_improvement"
        elif enough_time and median < -10 and negative >= 8:
            verdict = "locally_consistent_regression"
        result["comparisons"].append({
            "bytes": size, "fanout": width, "mix": mix, "kind": kind,
            "paired_improvement_percent": distribution(paired),
            "pairs_above_5_percent": positive, "pairs_below_minus_5_percent": negative,
            "both_medians_at_least_5_ms": enough_time, "verdict": verdict,
        })
    return result


def validate(directory):
    manifest = json.loads((directory / "manifest.json").read_text())
    for relative, expected in manifest["artifacts_sha256"].items():
        assert sha(directory / relative) == expected, relative
    for path, expected in manifest["inputs_sha256"].items():
        assert sha(ROOT / path) == expected, path
    assert sha(Path(manifest["compiler"])) == manifest["compiler_sha256"]
    assert sha(Path(manifest["binary"])) == manifest["binary_sha256"]
    assert all(command["returncode"] == 0 for command in manifest["commands"]
               if command["required"])
    summary = summarize(parse_rows(directory))
    assert summary == json.loads((directory / "summary.json").read_text())
    assert len(summary["configurations"]) == 63
    assert len(summary["comparisons"]) == 36
    print(json.dumps({"validation": "passed", "configurations": 63, "processes": 9,
                      "timed_rows": 567, "comparisons": 36, "directory": str(directory)}))


def run(args):
    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    directory = HERE / "runs" / (args.run_id or stamp)
    directory.mkdir(parents=True, exist_ok=False)
    build = ROOT / ".tools/static-payload-costs" / directory.name
    build.mkdir(parents=True, exist_ok=False)
    binary = build / "static-payload-costs"
    allowed = sorted(os.sched_getaffinity(0))
    cpu = 4 if 4 in allowed else allowed[0]
    manifest = {
        "protocol": "evidence/static-payload-review/protocol.md",
        "started_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "cwd": str(ROOT), "python": sys.version, "platform": platform.platform(),
        "uname": list(platform.uname()), "cpu_allowed_before_pinning": allowed,
        "benchmark_affinity": [cpu], "sample_count": SAMPLES,
        "load_average_before": os.getloadavg(),
        "compiler": str(RUSTC), "compiler_sha256": sha(RUSTC),
        "binary": str(binary), "commands": [],
        "environment_keys": sorted(os.environ),
        "selected_environment": {key: os.environ.get(key) for key in (
            "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "LD_PRELOAD",
            "LD_LIBRARY_PATH", "MALLOC_CONF", "MALLOC_ARENA_MAX", "LANG", "LC_ALL")},
        "inputs_sha256": {str(path.relative_to(ROOT)): sha(path) for path in (
            SOURCE, HERE / "run.py", HERE / "protocol.md")},
    }
    cpu_base = Path(f"/sys/devices/system/cpu/cpu{cpu}")
    manifest["cpu_sysfs"] = {}
    for relative in ("topology/core_id", "topology/physical_package_id",
                     "topology/thread_siblings_list", "cpufreq/scaling_governor",
                     "cpufreq/scaling_driver", "cpufreq/scaling_min_freq", "cpufreq/scaling_max_freq"):
        path = cpu_base / relative
        manifest["cpu_sysfs"][relative] = path.read_text().strip() if path.exists() else None

    def execute(label, command, required=True, affinity=None, timeout=120):
        begin = time.monotonic()
        record = {"label": label, "argv": [str(part) for part in command],
                  "required": required, "affinity": affinity, "timeout_seconds": timeout,
                  "started_utc": dt.datetime.now(dt.timezone.utc).isoformat()}
        try:
            proc = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=timeout,
                                  preexec_fn=(lambda: os.sched_setaffinity(0, affinity)) if affinity else None)
            stdout, stderr, returncode = proc.stdout, proc.stderr, proc.returncode
        except subprocess.TimeoutExpired as error:
            stdout, stderr, returncode = error.stdout or b"", error.stderr or b"", "timeout"
        except FileNotFoundError as error:
            stdout, stderr, returncode = b"", str(error).encode(), "unavailable"
        (directory / f"{label}.stdout").write_bytes(stdout)
        (directory / f"{label}.stderr").write_bytes(stderr)
        record.update(returncode=returncode, harness_elapsed_seconds=time.monotonic() - begin)
        manifest["commands"].append(record)
        dump(directory / "manifest.in-progress.json", manifest)
        if required and returncode != 0:
            raise RuntimeError(f"{label} failed: {returncode}; evidence retained in {directory}")
        return stdout

    execute("compiler", [RUSTC, "-vV"])
    execute("lscpu", ["lscpu", "--json"], required=False)
    execute("compile", [RUSTC, SOURCE, "--edition=2021", "-C", "opt-level=3",
                         "-C", "codegen-units=1", "-C", "lto=fat", "-C", "panic=abort",
                         "-C", "target-cpu=x86-64", "-C", "debuginfo=0", "-o", binary])
    manifest["binary_sha256"] = sha(binary)
    execute("binary-sections", ["size", binary], required=False)
    perf = shutil.which("perf")
    if perf:
        execute("performance-counter-probe", [perf, "stat", "-e",
                "cycles,instructions,cache-misses,branch-misses,task-clock,context-switches,cpu-migrations",
                "--", "true"], required=False, affinity=[cpu])
    else:
        manifest["performance_counter_status"] = "perf executable unavailable"
    print(f"Compiled; starting nine fixed independent processes on CPU{cpu}: {directory}", flush=True)
    for sample in range(SAMPLES):
        execute(f"sample-{sample:02}", [binary, str(sample)], affinity=[cpu])
        print(f"sample {sample + 1}/{SAMPLES} complete", flush=True)
    assert sha(binary) == manifest["binary_sha256"]
    for path, expected in manifest["inputs_sha256"].items():
        assert sha(ROOT / path) == expected, f"input changed during measurement: {path}"
    summary = summarize(parse_rows(directory))
    dump(directory / "summary.json", summary)
    with (directory / "summary.tsv").open("w") as file:
        writer = csv.writer(file, delimiter="\t")
        writer.writerow(["bytes", "fanout", "mix", "kind", "median_ns_per_view", "p25", "p75", "min", "max"])
        for config in summary["configurations"]:
            dist = config["ns_per_view"]
            writer.writerow([config[k] for k in ("bytes", "fanout", "mix", "kind")] +
                            [dist[k] for k in ("median", "p25", "p75", "min", "max")])
    manifest["finished_utc"] = dt.datetime.now(dt.timezone.utc).isoformat()
    manifest["load_average_after"] = os.getloadavg()
    manifest["artifacts_sha256"] = {
        str(path.relative_to(directory)): sha(path)
        for path in sorted(directory.iterdir())
        if path.name not in ("manifest.json", "manifest.in-progress.json")
    }
    dump(directory / "manifest.json", manifest)
    validate(directory)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--run-id", help="new directory name; existing evidence is never overwritten")
    parser.add_argument("--validate", type=Path, help="read-only validation of an existing run")
    args = parser.parse_args()
    if args.validate:
        validate(args.validate.resolve())
    else:
        run(args)
