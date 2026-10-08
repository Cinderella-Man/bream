#!/usr/bin/env python3
"""Bounded reference and Rust type-system observations, not benchmarks."""
from pathlib import Path
import hashlib
import json
import os
import platform
import resource
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
BASE = Path(__file__).resolve().parent
BUILD = ROOT / 'upstream/otp-reference-build'
PIN = 'ad05823719d77c8faee87348ea39513d4e2f99c5'
RUSTC = Path('/home/bat/projects/rust-actor-runtime-lab/.tools/rustup/toolchains/1.98.1-x86_64-unknown-linux-gnu/bin/rustc')
FAMILIES = ['cached_vs_dynamic', 'named_monitor_incarnation', 'missing_named_monitor',
            'missing_address_forms', 'detached_name_snapshot', 'timer_late_binding',
            'competing_names', 'competing_targets']


def record(path):
    return {'path': str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path),
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}


def no_core():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


commands = []
env = dict(os.environ)
for key in ['ERL_FLAGS', 'ERL_AFLAGS', 'ERL_ZFLAGS', 'ERL_COMPILER_OPTIONS']:
    env[key] = ''


def run(name, argv, directory):
    directory.mkdir(parents=True, exist_ok=True)
    start = time.monotonic()
    try:
        proc = subprocess.run([str(a) for a in argv], cwd=ROOT, env=env,
                              capture_output=True, timeout=45, preexec_fn=no_core)
        code, out, err, timeout = proc.returncode, proc.stdout, proc.stderr, False
    except subprocess.TimeoutExpired as exc:
        code, out, err, timeout = None, exc.stdout or b'', exc.stderr or b'', True
    stdout, stderr = directory / (name + '.stdout'), directory / (name + '.stderr')
    stdout.write_bytes(out)
    stderr.write_bytes(err)
    row = {'name': name, 'argv': [str(a) for a in argv], 'cwd': str(ROOT),
           'exit_code': code, 'timed_out': timeout, 'watchdog_seconds': 45,
           'elapsed_seconds_not_benchmark': round(time.monotonic() - start, 4),
           'stdout': record(stdout), 'stderr': record(stderr)}
    commands.append(row)
    return row, out.decode(errors='replace'), err.decode(errors='replace')


head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=BUILD, text=True).strip()
changes = subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=no'], cwd=BUILD, text=True).strip()
assert head == PIN and not changes, 'Reference build source pin/cleanliness mismatch'
prior_build = json.loads((ROOT / 'evidence/process-protocol-review/source-build.json').read_text())
for binary in prior_build['binary_hashes']:
    assert record(ROOT / binary['path'])['sha256'] == binary['sha256'], binary['path']

results, artifacts = [], []
for profile in ['jit-1', 'jit-4', 'debug-emu-4']:
    directory = BASE / 'runs' / profile
    directory.mkdir(parents=True, exist_ok=True)
    flags = ['+S', '1:1' if profile == 'jit-1' else '4:4', '+SDcpu', '1', '+SDio', '1']
    if profile == 'debug-emu-4':
        flags = ['-emu_type', 'debug', '-emu_flavor', 'emu'] + flags
    compile_row, _, _ = run('compile', [BUILD / 'bin/erlc', '+debug_info', '-o', directory,
                                       BASE / 'name_routing_probe.erl'], directory)
    assert compile_row['exit_code'] == 0 and not compile_row['timed_out'], compile_row
    beam = directory / 'name_routing_probe.beam'
    artifacts.append(record(beam))
    row, out, err = run('reference', [BUILD / 'bin/erl', *flags, '-noshell', '-pa', directory,
                                    '-s', 'name_routing_probe', 'run'], directory)
    observed = [line[5:] for line in out.splitlines() if line.startswith('PASS ')]
    ok = row['exit_code'] == 0 and not row['timed_out'] and not err and observed == FAMILIES
    ok = ok and 'COMPLETE families=8 race_rounds=100' in out
    results.append({'kind': 'pinned_beam', 'profile': profile, 'expected_families': FAMILIES,
                    'observed_families': observed, 'race_rounds': 100, 'expected_observation': ok})

version_row, version, _ = run('rustc-version', [RUSTC, '--version', '--verbose'], BASE / 'runs')
assert version_row['exit_code'] == 0 and 'rustc 1.98.1' in version
for opt in ['0', '3']:
    directory = BASE / 'runs' / ('rust-opt-' + opt)
    directory.mkdir(parents=True, exist_ok=True)
    binary = ROOT / '.tools' / 'name-routing-review' / ('variance-opt-' + opt)
    binary.parent.mkdir(parents=True, exist_ok=True)
    row, _, _ = run('variance-compile', [RUSTC, '--edition=2021', '-C', 'opt-level=' + opt,
                                        BASE / 'variance_probe.rs', '-o', binary], directory)
    assert row['exit_code'] == 0 and not row['timed_out'], row
    artifacts.append(record(binary))
    row, out, err = run('variance-execute', [binary], directory)
    accepted = row['exit_code'] == 0 and not row['timed_out'] and not err and out == 'COVARIANCE_BYPASSED_CONSTRUCTION_CHECK\n'
    results.append({'kind': 'rust_covariance_counterexample', 'opt_level': opt, 'expected_observation': accepted})
    row, _, err = run('invariant-reject', [RUSTC, '--edition=2021', '-C', 'opt-level=' + opt,
                                          BASE / 'invariant_reject.rs', '-o', binary.with_name('rejected-' + opt)], directory)
    rejected = row['exit_code'] == 1 and not row['timed_out'] and 'mismatched types' in err
    results.append({'kind': 'rust_invariance_compile_rejection', 'opt_level': opt, 'expected_observation': rejected})

manifest = {'date': '2026-10-08', 'source_commit': head, 'build_tracked_clean': not changes,
            'platform': platform.platform(), 'environment_overrides': {k: env[k] for k in
             ['ERL_FLAGS', 'ERL_AFLAGS', 'ERL_ZFLAGS', 'ERL_COMPILER_OPTIONS']},
            'scope': 'Eight targeted local BEAM families in three VM profiles and Rust variance controls in two optimization profiles; not exhaustive concurrency, Bream conformance or speed measurements.',
            'commands': commands, 'results': results,
            'all_expected_observations': all(r['expected_observation'] for r in results),
            'inputs': [record(BASE / p) for p in ['name_routing_probe.erl', 'variance_probe.rs', 'invariant_reject.rs', 'run_probes.py']],
            'tools': [record(ROOT / b['path']) for b in prior_build['binary_hashes']] + [record(RUSTC)],
            'artifacts': artifacts}
(BASE / 'probe-results.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps({'all_expected_observations': manifest['all_expected_observations'], 'results': results}, indent=2))
raise SystemExit(not manifest['all_expected_observations'])
