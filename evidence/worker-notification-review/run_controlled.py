#!/usr/bin/env python3
"""Separate matched-arrival follow-up. Never overwrites first-pass evidence."""
from pathlib import Path
import hashlib
import json
import os
import platform
import resource
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[2]
BASE = Path(__file__).resolve().parent
RUNS = BASE / 'controlled-runs'
RUNS.mkdir(exist_ok=True)
SOURCE = ROOT / 'experiments/worker-notification/src/controlled.rs'
RUSTC = Path('/home/bat/projects/rust-actor-runtime-lab/.tools/rustup/toolchains/1.98.1-x86_64-unknown-linux-gnu/bin/rustc')
BINARY = ROOT / '.tools/worker-notification/controlled-notification-probe'
POLICIES = ['condvar_always', 'condvar_coalesced', 'park_always', 'park_coalesced']
WORKLOADS = ['rate500k', 'rate1m', 'rate2m']


def rec(path):
    return {'path': str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path),
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}


def setup():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    os.sched_setaffinity(0, [0, 1])


commands = []


def run(name, argv):
    try:
        r = subprocess.run([str(a) for a in argv], cwd=ROOT, capture_output=True,
                           timeout=30, preexec_fn=setup)
        code, out, err, timeout = r.returncode, r.stdout, r.stderr, False
    except subprocess.TimeoutExpired as exc:
        code, out, err, timeout = None, exc.stdout or b'', exc.stderr or b'', True
    stdout, stderr = RUNS / (name + '.stdout'), RUNS / (name + '.stderr')
    stdout.write_bytes(out)
    stderr.write_bytes(err)
    row = {'name': name, 'argv': [str(a) for a in argv], 'cwd': str(ROOT),
           'exit_code': code, 'timed_out': timeout, 'watchdog_seconds': 30,
           'stdout': rec(stdout), 'stderr': rec(stderr)}
    commands.append(row)
    assert code == 0 and not timeout and not err, row
    return out.decode()


protocol = {'reason': 'First saturated comparison changes offered rate when publication becomes cheaper; do not interpret its queue-delay difference as a scheduler regression.',
            'samples': 12, 'warmups': 1, 'batch_size': 8, 'batches': 16384,
            'scheduled_batch_rates_per_second': [500000, 1000000, 2000000],
            'arrival_clock': 'Fixed absolute Instant schedule; late producer catches up. Record issue lateness and scheduled-to-claim p99.',
            'order': 'Rotate policy order by sample index, reverse each fourth block, rotate workloads by sample index.',
            'decision': 'Report paired median ratios and per-arm min/median/max; select no backend from these short, noisy host cohorts. Require responsiveness evidence on the admitted process engine.',
            'scope': 'Same queue/enrollment/backend logic as first probe, with one added scheduled timestamp and per-batch latency vector. Compare within this run only.',
            'affinity': [0, 1], 'placement': 'OS may place either thread on either CPU; no per-thread pin.',
            'not_measured': ['process kill', 'application preemption', 'I/O integration', 'reclamation', 'energy', 'syscalls', 'hardware counters']}
(BASE / 'controlled-protocol.json').write_text(json.dumps(protocol, indent=2) + '\n')
first_result = rec(BASE / 'results.json')
run('build', [RUSTC, '--edition=2021', '-C', 'opt-level=3', SOURCE, '-o', BINARY])
rows = []
for sample in range(-1, 12):
    policies = POLICIES[sample % 4:] + POLICIES[:sample % 4]
    if sample % 4 == 3:
        policies = list(reversed(policies))
    for workload in WORKLOADS[sample % 3:] + WORKLOADS[:sample % 3]:
        for policy in policies:
            name = f'{"warmup" if sample < 0 else f"sample-{sample:02d}"}-{workload}-{policy}'
            row = json.loads(run(name, [BINARY, policy, workload]))
            assert row['correct'] and row['policy'] == policy and row['workload'] == workload
            assert row['items'] == 131072 and row['batches'] == 16384
            rows.append({'sample': sample, 'warmup': sample < 0, 'command': name, **row})
groups = []
for workload in WORKLOADS:
    for policy in POLICIES:
        selected = [r for r in rows if not r['warmup'] and r['workload'] == workload and r['policy'] == policy]
        assert len(selected) == 12
        metrics = {}
        for key in ['elapsed_ns', 'notify_calls', 'wait_calls', 'queue_delay_p99_ns',
                    'queue_delay_max_ns', 'scheduled_to_claim_p99_ns', 'issue_lateness_p99_ns']:
            values = [r[key] for r in selected]
            metrics[key] = {'min': min(values), 'median': statistics.median(values), 'max': max(values)}
        groups.append({'workload': workload, 'policy': policy, 'metrics': metrics})
comparisons = []
for workload in WORKLOADS:
    keyed = {(r['policy'], r['sample']): r for r in rows if not r['warmup'] and r['workload'] == workload}
    for baseline, treatment in [('condvar_always', 'condvar_coalesced'), ('park_always', 'park_coalesced'),
                                ('condvar_always', 'park_always'), ('condvar_coalesced', 'park_coalesced')]:
        comparisons.append({'workload': workload, 'baseline': baseline, 'treatment': treatment,
                            'paired_median_ratios': {k: statistics.median(
                                keyed[(treatment, n)][k] / keyed[(baseline, n)][k] for n in range(12))
                                for k in ['elapsed_ns', 'queue_delay_p99_ns', 'scheduled_to_claim_p99_ns', 'issue_lateness_p99_ns']}})
assert rec(BASE / 'results.json') == first_result, 'Initial results were changed'
manifest = {'date': '2026-10-08', 'platform': platform.platform(), 'protocol': rec(BASE / 'controlled-protocol.json'),
            'first_run_preserved': first_result, 'inputs': [rec(SOURCE), rec(Path(__file__))],
            'tool': rec(RUSTC), 'artifact': rec(BINARY), 'commands': commands,
            'measured_invocations': 144, 'warmup_invocations': 12, 'all_checks_passed': True,
            'measurements': rows, 'groups': groups, 'comparisons': comparisons,
            'load_after': Path('/proc/loadavg').read_text()}
(BASE / 'controlled-results.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps({'all_checks_passed': True, 'comparisons': comparisons}, indent=2))
