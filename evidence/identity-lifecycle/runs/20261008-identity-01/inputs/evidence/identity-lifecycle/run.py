#!/usr/bin/env python3
"""Build and retain one immutable identity/lifecycle milestone evidence run."""
from pathlib import Path
from collections import defaultdict
import hashlib
import json
import os
import platform
import resource
import signal
import shutil
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
BASE = Path(__file__).resolve().parent
PROJECT = ROOT / 'experiments/identity-lifecycle'
TOOLCHAIN = Path('/home/bat/projects/rust-actor-runtime-lab/.tools/rustup/toolchains/1.98.1-x86_64-unknown-linux-gnu/bin')
RUN = BASE / 'runs' / (sys.argv[1] if len(sys.argv) > 1 else time.strftime('%Y%m%d-%H%M%S', time.gmtime()))
RUN.mkdir(parents=True, exist_ok=False)
TARGET = ROOT / '.tools/identity-lifecycle' / RUN.name
ENV = {k: v for k, v in os.environ.items() if not k.startswith('LOOM_')}
ENV.pop('CARGO_ENCODED_RUSTFLAGS', None)
OVERRIDES = {'CARGO_HOME': str(ROOT / '.tools/cargo-cache'), 'CARGO_TARGET_DIR': str(TARGET),
             'RUSTC': str(TOOLCHAIN / 'rustc'), 'RUSTFLAGS': '', 'RUSTDOCFLAGS': '',
             'RUSTC_WRAPPER': '', 'RUSTC_WORKSPACE_WRAPPER': '', 'RUST_BACKTRACE': '0'}
ENV.update(OVERRIDES)
AFFINITY = [0, 1]
assert set(AFFINITY) <= os.sched_getaffinity(0)
commands = []
observations = []
errors = []


def record(path):
    path = Path(path)
    return {'path': str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path),
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}


def setup():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    os.sched_setaffinity(0, AFFINITY)


def retained_input(path):
    target = RUN / 'inputs' / path.relative_to(ROOT)
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(path, target)
    return {'original_path': str(path.relative_to(ROOT)), **record(target)}


def invoke(name, argv, timeout=30):
    begin = time.monotonic()
    child = subprocess.Popen([str(a) for a in argv], cwd=ROOT, env=ENV,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             start_new_session=True, preexec_fn=setup)
    expired = False
    try:
        stdout, stderr = child.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        expired = True
        os.killpg(child.pid, signal.SIGKILL)
        stdout, stderr = child.communicate()
    out, err = RUN / (name + '.stdout'), RUN / (name + '.stderr')
    out.write_bytes(stdout)
    err.write_bytes(stderr)
    command = {'name': name, 'argv': [str(a) for a in argv], 'exit_code': child.returncode,
               'timed_out': expired, 'watchdog_seconds': timeout, 'wall_seconds': time.monotonic()-begin,
               'stdout': record(out), 'stderr': record(err)}
    commands.append(command)
    return command, stdout.decode(), stderr.decode()


REFERENCE = ROOT / '.tools/structured-process/20261008-structured-01/release/bream-structured-consumer'
protocol = {
    'date': '2026-10-08', 'milestone': 'Generational process identity, terminal arbitration and publication/shutdown ownership',
    'allowances': [1, 32, 256], 'samples_per_configuration': 12, 'warmups_per_configuration': 1,
    'correctness': '11 deterministic runtime transition/failure tests per compiler profile; identity, structured and baseline controls at three allowances per profile; 57 rejected consumers per profile; four shadowing executions.',
    'kill_races': '200 public host kill/completion races per control invocation, alternating 8 words/two rounds and 256 words/16 rounds. Receipt determines required terminal outcome; either arrival order is allowed.',
    'shutdown': 'Publication before seal belongs to runtime; after seal destroys prepared input and returns Closed. Shutdown joins accepted worker work, not in-progress host preparation.',
    'comparisons': {
        'registry': 'Freshly execute preserved structured-runtime binary and new structured consumer on identical typed-state workload. Twelve paired samples per allowance; rotate allowances across three positions and alternate old/new order, six each.',
        'churn': '128 sequential 64-word/two-round processes reuse one slot beside an infinite 128-word peer; retain stale IDs and kill peer at end. Twelve samples per allowance, cyclic position balance.',
        'lookup': 'One million repeated live or stale ID queries: lookup(id).is_ok() retains a handle, is_live(id) borrows under the guard. Four configurations rotate through each position three times. Uncontended microbenchmark, not message-send throughput.',
    },
    'decision': 'Keep all raw samples. Report min/median/max and paired ratios; no significance or hard latency guarantee, no production allowance selected. Responsiveness first.',
    'affinity': AFFINITY, 'placement': 'Two-CPU inherited mask; one application worker plus host, OS selects placement. Controls also create host publisher/killer threads.',
    'development': 'Unrecorded development builds and smoke checks preceded this fixed retained protocol.',
    'limits': ['single worker', 'closed generated grammar', 'recoverable policy/capacity/channel failures only; no allocator-failure recovery',
               'worker failure is host-fatal and unrecovered', 'no waiting I/O/mailboxes/monitors', 'no complete G1/G2 qualification'],
}
(RUN / 'protocol.json').write_text(json.dumps(protocol, indent=2)+'\n')
inputs = [retained_input(p) for p in sorted(PROJECT.rglob('*')) if p.is_file() and p.suffix in ['.rs', '.toml', '.lock']]
inputs.append(retained_input(Path(__file__).resolve()))
artifacts = [record(REFERENCE)]
try:
    c, out, err = invoke('rustc-version', [TOOLCHAIN / 'rustc', '-vV'])
    assert c['exit_code'] == 0 and 'rustc 1.98.1' in out
    common = ['--offline', '--locked', '--manifest-path', PROJECT / 'Cargo.toml']
    build = [TOOLCHAIN / 'cargo', 'build', *common, '-p', 'bream-identity-consumer']
    negatives = sorted((PROJECT / 'consumer/examples').glob('reject_*.rs'))
    diagnostic = {
        'reject_future': 'mismatched types', 'reject_closure_spawn': 'mismatched types',
        'reject_safe_constructor': 'requires unsafe function or block', 'reject_program_fields': 'is private',
        'reject_drop_input': 'mismatched types', 'reject_struct_duplicate_owner': 'E0382',
        'reject_struct_use_after_move': 'E0382', 'reject_struct_missing_arm': 'E0004',
        'reject_struct_private_type': 'E0603', 'reject_struct_private_helper': 'E0603',
        'reject_id_fields': 'E0451', 'reject_id_mutation': 'E0616',
    }
    for profile in ['debug', 'release']:
        flags = ['--release'] if profile == 'release' else []
        c, out, err = invoke('unit-' + profile, [TOOLCHAIN / 'cargo', 'test', *common, '-p', 'bream-identity-runtime', '--lib', *flags], 60)
        assert c['exit_code'] == 0 and '11 passed; 0 failed' in out, (out, err)
        observations.append({'kind': 'unit_tests', 'command': c['name'], 'profile': profile, 'tests': 11, 'passed': True})
        c, out, err = invoke('build-' + profile, build + flags, 60)
        assert c['exit_code'] == 0 and not c['timed_out'], err
        for family, name in [('identity', 'bream-identity-consumer'), ('structured', 'structured'), ('baseline', 'baseline')]:
            binary = TARGET / profile / name
            artifacts.append(record(binary))
            for allowance in protocol['allowances']:
                c, out, err = invoke(f'controls-{family}-{profile}-{allowance}', [binary, 'controls', allowance])
                assert c['exit_code'] == 0 and not c['timed_out'], err
                result = json.loads(out)
                assert result['passed'] and 'index out of bounds' in err
                if family == 'identity':
                    assert result['kill_accepted'] + result['kill_too_late'] == 200 and result['live_after_shutdown'] == 0
                if family == 'structured':
                    assert 'input already taken' in err and result['shared_budget_steps'] == 6
                observations.append({'command': c['name'], 'profile': profile, 'family': family, **result})
        for example, expected in [('shadow_paths', 'SHADOW_PATHS_PASSED'), ('shadow_structured', 'SHADOW_STRUCTURED_PASSED')]:
            c, _, err = invoke(example + '-build-' + profile, build + flags + ['--example', example], 60)
            assert c['exit_code'] == 0 and not c['timed_out'], err
            shadow = TARGET / profile / 'examples' / example
            artifacts.append(record(shadow))
            c, out, err = invoke(example + '-execute-' + profile, [shadow])
            assert c['exit_code'] == 0 and out == expected + '\n' and not err
        for fixture in negatives:
            c, out, err = invoke('reject-' + profile + '-' + fixture.stem, build + flags + ['--example', fixture.stem], 60)
            required = diagnostic.get(fixture.stem, 'bream structured subset:' if fixture.stem.startswith('reject_struct_') else 'bream milestone subset:')
            assert c['exit_code'] != 0 and not c['timed_out'] and required in err, (fixture.name, err)
            observations.append({'kind': 'rejection', 'command': c['name'], 'profile': profile, 'fixture': fixture.stem, 'diagnostic': required, 'passed': True})
    binary = TARGET / 'release/bream-identity-consumer'
    structured = TARGET / 'release/structured'
    samples = range(-1, protocol['samples_per_configuration'])
    for sample in samples:
        order = protocol['allowances'][:]
        offset = sample % 3
        order = order[offset:] + order[:offset]
        for allowance in order:
            variants = [('reference', REFERENCE), ('registry', structured)]
            if sample % 2: variants.reverse()
            for variant, executable in variants:
                name = f'paired-{sample:02d}-{allowance}-{variant}'
                c, out, err = invoke(name, [executable, 'measure', allowance])
                assert c['exit_code'] == 0 and not c['timed_out'] and not err, (name, err)
                result = json.loads(out)
                assert result['passed'] and result['peer_digest'] == 5468279037442777099
                assert result['victim_drops'] == 1 and result['no_post_stop_resume']
                observations.append({'command': name, 'sample': sample, 'warmup': sample == -1, 'experiment': 'registry', 'variant': variant, **result})
    for sample in samples:
        order = protocol['allowances'][:]
        offset = sample % 3
        order = order[offset:] + order[:offset]
        for allowance in order:
            name = f'churn-{sample:02d}-{allowance}'
            c, out, err = invoke(name, [binary, 'measure', allowance])
            assert c['exit_code'] == 0 and not c['timed_out'] and not err, (name, err)
            result = json.loads(out)
            assert result['passed'] and result['processes'] == 128 and result['victim_drops'] == 1
            observations.append({'command': name, 'sample': sample, 'warmup': sample == -1, 'experiment': 'churn', **result})
    configurations = [('retained', 'live'), ('borrowed', 'live'), ('retained', 'stale'), ('borrowed', 'stale')]
    for sample in samples:
        offset = sample % 4
        order = configurations[offset:] + configurations[:offset]
        for mode, state in order:
            name = f'lookup-{sample:02d}-{mode}-{state}'
            c, out, err = invoke(name, [binary, 'lookup', mode, state])
            assert c['exit_code'] == 0 and not c['timed_out'] and not err, (name, err)
            result = json.loads(out)
            assert result['passed'] and result['hits'] == (result['iterations'] if state == 'live' else 0)
            observations.append({'command': name, 'sample': sample, 'warmup': sample == -1, 'experiment': 'lookup', **result})
except Exception as error:
    errors.append(repr(error))

def group_key(row):
    if row['experiment'] == 'registry': return ('registry', row['variant'], row['allowance'])
    if row['experiment'] == 'churn': return ('churn', row['allowance'])
    return ('lookup', row['mode'], row['state'])

measured = [r for r in observations if 'experiment' in r and not r['warmup']]
grouped = defaultdict(list)
for row in measured: grouped[group_key(row)].append(row)
metrics = {
    'registry': ['peer_completion_ns', 'peer_first_poll_ns', 'peer_ready_delay_max_ns', 'kill_to_stop_ns', 'stop_to_cleanup_ns'],
    'churn': ['completion_ns', 'peer_first_poll_max_ns', 'peer_ready_delay_max_ns', 'kill_to_stop_ns', 'stop_to_cleanup_ns'],
    'lookup': ['elapsed_ns'],
}
groups = []
for key, rows in grouped.items():
    summaries = {}
    for metric in metrics[key[0]]:
        values = [r[metric] for r in rows]
        summaries[metric] = {'min': min(values), 'median': statistics.median(values), 'max': max(values)}
    groups.append({'key': key, 'samples': len(rows), 'metrics': summaries})
comparisons = []
for allowance in protocol['allowances']:
    ref = {r['sample']: r for r in grouped[('registry', 'reference', allowance)]}
    treatment = {r['sample']: r for r in grouped[('registry', 'registry', allowance)]}
    if len(ref) == len(treatment) == 12:
        ratios = [ref[s]['peer_completion_ns'] / treatment[s]['peer_completion_ns'] for s in range(12)]
        comparisons.append({'experiment': 'registry', 'allowance': allowance, 'metric': 'peer_completion_ns',
                            'baseline_over_treatment_ratios': ratios, 'median': statistics.median(ratios)})
for state in ['live', 'stale']:
    ref = {r['sample']: r for r in grouped[('lookup', 'retained', state)]}
    treatment = {r['sample']: r for r in grouped[('lookup', 'borrowed', state)]}
    if len(ref) == len(treatment) == 12:
        ratios = [ref[s]['elapsed_ns'] / treatment[s]['elapsed_ns'] for s in range(12)]
        comparisons.append({'experiment': 'lookup', 'state': state, 'metric': 'elapsed_ns',
                            'baseline_over_treatment_ratios': ratios, 'median': statistics.median(ratios)})
result = {
    'status': 'failed' if errors else 'passed', 'errors': errors, 'protocol': record(RUN / 'protocol.json'),
    'inputs': inputs, 'tools': [record(TOOLCHAIN / t) for t in ['rustc', 'cargo']], 'artifacts': artifacts,
    'reference_provenance': record(ROOT / 'evidence/structured-process/runs/20261008-structured-01/results.json'),
    'commands': commands, 'observations': observations, 'groups': groups, 'comparisons': comparisons,
    'host': {'platform': platform.platform(), 'cpu_count': os.cpu_count(), 'affinity': AFFINITY, 'load_after': Path('/proc/loadavg').read_text().strip()},
    'environment_overrides': OVERRIDES, 'new_beam_execution': False, 'production_runtime': False,
}
(RUN / 'results.json').write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps({'status': result['status'], 'run': str(RUN), 'commands': len(commands), 'measurements': len(measured), 'comparisons': comparisons, 'errors': errors}, indent=2))
raise SystemExit(bool(errors))

