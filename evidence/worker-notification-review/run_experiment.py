#!/usr/bin/env python3
"""Retain small Loom controls and paired worker-notification observations."""
from pathlib import Path
import hashlib
import json
import os
import platform
import random
import re
import resource
import signal
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
BASE = Path(__file__).resolve().parent
PROJECT = ROOT / 'experiments/worker-notification'
TOOLS = Path('/home/bat/projects/rust-actor-runtime-lab/.tools/rustup/toolchains/1.98.1-x86_64-unknown-linux-gnu/bin')
TARGET = ROOT / '.tools/worker-notification'
RUNS = BASE / 'runs'
RUNS.mkdir(parents=True, exist_ok=True)
POLICIES = ['condvar_always', 'condvar_coalesced', 'park_always', 'park_coalesced']
WORKLOADS = ['saturated', 'sparse', 'burst']
SAMPLES = 12
AFFINITY = [0, 1]
ENV = {k: v for k, v in os.environ.items() if not k.startswith('LOOM_')}
ENV.pop('CARGO_ENCODED_RUSTFLAGS', None)
OVERRIDES = {'RUSTC': str(TOOLS / 'rustc'), 'RUSTFLAGS': '', 'RUSTDOCFLAGS': '',
             'RUSTC_WRAPPER': '', 'RUSTC_WORKSPACE_WRAPPER': '', 'RUST_BACKTRACE': '0',
             'CARGO_HOME': str(ROOT / '.tools/cargo-cache'), 'CARGO_TARGET_DIR': str(TARGET)}
ENV.update(OVERRIDES)
assert set(AFFINITY) <= os.sched_getaffinity(0)


def record(path):
    return {'path': str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path),
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}


def setup():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    os.sched_setaffinity(0, AFFINITY)


commands = []


def invoke(name, argv, timeout=30):
    start = time.monotonic()
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    child = subprocess.Popen([str(a) for a in argv], cwd=ROOT, env=ENV,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             start_new_session=True, preexec_fn=setup)
    timed_out = False
    try:
        out, err = child.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(child.pid, signal.SIGKILL)
        out, err = child.communicate()
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    stdout, stderr = RUNS / (name + '.stdout'), RUNS / (name + '.stderr')
    stdout.write_bytes(out)
    stderr.write_bytes(err)
    row = {'name': name, 'argv': [str(a) for a in argv], 'cwd': str(ROOT),
           'exit_code': child.returncode, 'timed_out': timed_out, 'watchdog_seconds': timeout,
           'total_child_wall_seconds': time.monotonic() - start,
           'total_child_user_seconds': after.ru_utime - before.ru_utime,
           'total_child_system_seconds': after.ru_stime - before.ru_stime,
           'stdout': record(stdout), 'stderr': record(stderr)}
    commands.append(row)
    return row, out.decode(errors='replace'), err.decode(errors='replace')


# Saved before compilation or samples; criteria are exploratory, not a product SLO.
protocol = {
    'samples_per_policy_workload': SAMPLES, 'warmups_per_policy_workload': 1,
    'order': 'Rotate the four policies by sample index; reverse every fourth block. Workloads rotate by sample index.',
    'affinity': AFFINITY, 'per_thread_placement': 'Uncontrolled within the inherited two-CPU mask.',
    'primary': 'Saturated elapsed time; scheduled sparse/burst cohorts expose latency and wake-call tradeoffs.',
    'candidate_signal': 'Bootstrap 95% interval for paired elapsed treatment/baseline median entirely below 0.95, with queue-delay p99 median ratio <= 1.10. This is an exploratory cohort signal, never automatic acceptance.',
    'bootstrap': {'seed': 5701, 'resamples': 5000, 'statistic': 'median of paired ratios'},
    'correctness': 'Every measured process checks exact sequence, item count, empty final queue and independent checksum; deadlock/timeout fails. Close drains admitted work.',
    'limits': 'One producer and one worker; no Bream application preemption, kill, reclamation, I/O, multiple consumers, energy, syscall counts or performance counters. Queue delay is enqueue-to-claim, not pure OS wake latency.',
    'timing': 'Elapsed includes publication, worker activation, work and join; excludes allocation before start and checksum verification afterward. Each process is fresh; warmups use separate processes. Scheduled issue lateness is recorded.',
}
(BASE / 'protocol.json').write_text(json.dumps(protocol, indent=2) + '\n')
version, compiler, _ = invoke('rustc-version', [TOOLS / 'rustc', '-vV'])
assert version['exit_code'] == 0 and 'rustc 1.98.1' in compiler
build, _, err = invoke('build', [TOOLS / 'cargo', 'build', '--release', '--locked', '--offline',
                               '--manifest-path', PROJECT / 'Cargo.toml'], 120)
assert build['exit_code'] == 0 and not build['timed_out'], err
model = TARGET / 'release/notification-model'
probe = TARGET / 'release/notification-probe'
models = []
for name in ['condvar_locked', 'park_locked', 'park_split_enrollment', 'park_token_consumed']:
    row, out, err = invoke('model-' + name, [model, name])
    positive = name in ['condvar_locked', 'park_locked']
    count = re.search(r'executions_started=(\d+)', out) if positive else re.search(r'MODEL_EXECUTIONS_STARTED=(\d+)', err)
    ok = (row['exit_code'] == 0 and not err and f'model={name} completed=true' in out) if positive else (
        row['exit_code'] == -signal.SIGABRT and 'deadlock; threads =' in err and not out)
    ok = bool(ok and not row['timed_out'] and count)
    models.append({'name': name, 'positive': positive, 'expected_observation': ok,
                   'executions_started': int(count[1]) if count else None})
assert all(r['expected_observation'] for r in models), models

measurements = []
for sample in range(-1, SAMPLES):
    order = POLICIES[sample % 4:] + POLICIES[:sample % 4]
    if sample % 4 == 3:
        order = list(reversed(order))
    workloads = WORKLOADS[sample % 3:] + WORKLOADS[:sample % 3]
    for workload in workloads:
        for policy in order:
            stem = f'{"warmup" if sample < 0 else f"sample-{sample:02d}"}-{workload}-{policy}'
            row, out, err = invoke(stem, [probe, policy, workload])
            assert row['exit_code'] == 0 and not row['timed_out'] and not err, row
            observation = json.loads(out)
            assert observation['correct'] and observation['policy'] == policy and observation['workload'] == workload
            measurements.append({'sample': sample, 'warmup': sample < 0, 'command': stem, **observation})

groups = []
for workload in WORKLOADS:
    for policy in POLICIES:
        rows = [r for r in measurements if not r['warmup'] and r['workload'] == workload and r['policy'] == policy]
        assert len(rows) == SAMPLES and len({r['checksum'] for r in rows}) == 1
        metrics = {}
        for metric in ['elapsed_ns', 'notify_calls', 'wait_calls', 'queue_delay_p50_ns',
                       'queue_delay_p99_ns', 'queue_delay_max_ns', 'issue_lateness_p99_ns']:
            values = [r[metric] for r in rows]
            metrics[metric] = {'min': min(values), 'median': statistics.median(values), 'max': max(values)}
        groups.append({'workload': workload, 'policy': policy, 'samples': SAMPLES, 'metrics': metrics})

comparisons = []
rng = random.Random(5701)
for workload in WORKLOADS:
    for base, treatment in [('condvar_always', 'condvar_coalesced'), ('park_always', 'park_coalesced'),
                            ('condvar_always', 'park_always'), ('condvar_coalesced', 'park_coalesced')]:
        rows = {(r['policy'], r['sample']): r for r in measurements if not r['warmup'] and r['workload'] == workload}
        ratios = [rows[(treatment, n)]['elapsed_ns'] / rows[(base, n)]['elapsed_ns'] for n in range(SAMPLES)]
        boot = sorted(statistics.median(rng.choices(ratios, k=SAMPLES)) for _ in range(5000))
        latency_ratios = [rows[(treatment, n)]['queue_delay_p99_ns'] / rows[(base, n)]['queue_delay_p99_ns'] for n in range(SAMPLES)]
        latency = statistics.median(latency_ratios)
        ci = [boot[125], boot[4874]]
        comparisons.append({'workload': workload, 'baseline': base, 'treatment': treatment,
                            'paired_elapsed_ratio_median': statistics.median(ratios),
                            'bootstrap_95_percent_interval': ci,
                            'paired_p99_queue_delay_ratio_median': latency,
                            'exploratory_improvement_signal': ci[1] < 0.95 and latency <= 1.10})

host_paths = ['/proc/cpuinfo', '/proc/loadavg', '/sys/devices/system/cpu/cpu0/topology/thread_siblings_list',
              '/sys/devices/system/cpu/cpu1/topology/thread_siblings_list',
              '/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor']
host = {p: Path(p).read_text() if Path(p).is_file() else 'unavailable' for p in host_paths}
result = {'date': '2026-10-08', 'scope': protocol['limits'], 'platform': platform.platform(),
          'host_observation_after_samples': host, 'environment_overrides': OVERRIDES,
          'removed_environment': 'LOOM_* and CARGO_ENCODED_RUSTFLAGS',
          'protocol': record(BASE / 'protocol.json'),
          'builder': {'max_threads': 2, 'max_branches': 1000, 'max_permutations': None,
                      'max_duration': None, 'preemption_bound': None, 'checkpoint_file': None,
                      'location': True, 'log': False},
          'inputs': [record(PROJECT / p) for p in ['Cargo.toml', 'Cargo.lock', 'src/model.rs', 'src/probe.rs']] + [record(Path(__file__))],
          'tools': [record(TOOLS / 'rustc'), record(TOOLS / 'cargo')],
          'artifacts': [record(model), record(probe)], 'commands': commands,
          'models': models, 'all_expected_observations': True,
          'measured_invocations': SAMPLES * 12, 'warmup_invocations': 12,
          'measurements': measurements, 'groups': groups, 'comparisons': comparisons,
          'qualification': 'Exploratory host microbench and tiny protocol models, not selected Bream backend or production improvement.'}
(BASE / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'models': models, 'measured_invocations': result['measured_invocations'],
                  'comparisons': comparisons}, indent=2), flush=True)
