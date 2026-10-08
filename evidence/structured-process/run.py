#!/usr/bin/env python3
"""Build and retain one immutable structured-state/helper milestone evidence run."""
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
PROJECT = ROOT / 'experiments/structured-process'
TOOLCHAIN = Path('/home/bat/projects/rust-actor-runtime-lab/.tools/rustup/toolchains/1.98.1-x86_64-unknown-linux-gnu/bin')
RUN = BASE / 'runs' / (sys.argv[1] if len(sys.argv) > 1 else time.strftime('%Y%m%d-%H%M%S', time.gmtime()))
RUN.mkdir(parents=True, exist_ok=False)
TARGET = ROOT / '.tools/structured-process' / RUN.name
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


protocol = {
    'date': '2026-10-08', 'milestone': 'Private owning records, tagged enums, shared-budget checked helpers and partial-initialization cancellation',
    'allowances': [1, 32, 256], 'release_samples_per_allowance': 9, 'separate_warmup_processes_per_allowance': 1,
    'order': 'Three cyclic orders; each allowance occupies each ordinal three times across nine samples.',
    'work': '8192-word typed-state transform with nested helpers beside a nonterminating helper owning a 1024-word record; kill hog after peer completes, then run another peer.',
    'timing': 'Peer submission follows host input and frame allocation. Completion includes all worker scheduling and instrumented data work. Kill begins at host request timestamp; stop precedes dropping the suspended frame; cleanup is published after drop returns.',
    'metrics': ['peer_first_poll_ns', 'peer_ready_delay_max_ns', 'peer_completion_ns',
                'peer_frame_bytes', 'kill_to_stop_ns', 'stop_to_cleanup_ns'],
    'decision': 'Retain every measured run and report min/median/max and paired throughput ratios. No significance claim, numerical SLO, hard bound or production allowance is selected.',
    'correctness': 'Independent typed-state digest and enum branches; nested and ordered helpers; shared six-step allowance control; four partial-record/argument/helper ownership kill cases with peer progress; panic cleanup; empty/nested helper loops; baseline controls; separate-consumer rejection and two lexical shadowing routes.',
    'development': 'Unrecorded development compilation and debug smoke checks preceded this fixed release evidence protocol. They are not included as timing evidence.',
    'affinity': AFFINITY, 'placement': 'Inherited two-CPU mask; OS chooses per-thread placement. One application worker plus host control thread.',
    'limits': ['closed private record/enum/acyclic-helper grammar with owning arguments', 'trusted Rust dependencies and host allocator',
               'instrumentation affects costs', 'no hard wall-clock bound', 'no messages or waiting I/O',
               'no reusable identity registry', 'no complete G1/G2 qualification'],
}
(RUN / 'protocol.json').write_text(json.dumps(protocol, indent=2)+'\n')
inputs = [retained_input(p) for p in sorted(PROJECT.rglob('*')) if p.is_file() and p.suffix in ['.rs', '.toml', '.lock']]
inputs.append(retained_input(Path(__file__).resolve()))
artifacts = []
try:
    c, out, err = invoke('rustc-version', [TOOLCHAIN / 'rustc', '-vV'])
    assert c['exit_code'] == 0 and 'rustc 1.98.1' in out
    cargo = [TOOLCHAIN / 'cargo', 'build', '--offline', '--locked', '--manifest-path', PROJECT / 'Cargo.toml',
             '-p', 'bream-structured-consumer']
    negatives = sorted((PROJECT / 'consumer/examples').glob('reject_*.rs'))
    diagnostic = {
        'reject_future': 'mismatched types', 'reject_closure_spawn': 'mismatched types',
        'reject_safe_constructor': 'requires unsafe function or block',
        'reject_program_fields': 'is private', 'reject_drop_input': 'mismatched types',
    }
    diagnostic.update({
        'reject_struct_duplicate_owner': 'E0382', 'reject_struct_use_after_move': 'E0382',
        'reject_struct_missing_arm': 'E0004', 'reject_struct_private_type': 'E0603',
        'reject_struct_private_helper': 'E0603',
    })
    for profile in ['debug', 'release']:
        flags = ['--release'] if profile == 'release' else []
        c, _, err = invoke('build-' + profile, cargo + flags, 60)
        assert c['exit_code'] == 0 and not c['timed_out'], err
        binary = TARGET / profile / 'bream-structured-consumer'
        artifacts.append(record(binary))
        for family, executable in [('structured', binary), ('baseline', TARGET / profile / 'baseline')]:
            if family == 'baseline': artifacts.append(record(executable))
            for allowance in protocol['allowances']:
                c, out, err = invoke(f'controls-{family}-{profile}-{allowance}', [executable, 'controls', allowance])
                assert c['exit_code'] == 0 and not c['timed_out'], err
                result = json.loads(out)
                assert result['passed'] and 'index out of bounds' in err
                if family == 'structured':
                    assert 'input already taken' in err and result['shared_budget_steps'] == 6
                    assert len(result['kill_scenarios']) == 4
                observations.append({'command': c['name'], 'profile': profile, 'family': family, **result})
        for example, expected in [('shadow_paths', 'SHADOW_PATHS_PASSED'), ('shadow_structured', 'SHADOW_STRUCTURED_PASSED')]:
            c, _, err = invoke(example + '-build-' + profile, cargo + flags + ['--example', example], 60)
            assert c['exit_code'] == 0 and not c['timed_out'], err
            shadow = TARGET / profile / 'examples' / example
            artifacts.append(record(shadow))
            c, out, err = invoke(example + '-execute-' + profile, [shadow])
            assert c['exit_code'] == 0 and out == expected + '\n' and not err
        for fixture in negatives:
            c, out, err = invoke('reject-' + profile + '-' + fixture.stem, cargo + flags + ['--example', fixture.stem], 60)
            required = diagnostic.get(fixture.stem, 'bream structured subset:' if fixture.stem.startswith('reject_struct_') else 'bream milestone subset:')
            assert c['exit_code'] != 0 and not c['timed_out'] and required in err, (fixture.name, err)
            observations.append({'kind': 'rejection', 'command': c['name'], 'profile': profile,
                                 'fixture': fixture.stem, 'diagnostic': required, 'passed': True})
    binary = TARGET / 'release/bream-structured-consumer'
    for sample in range(-1, 9):
        order = protocol['allowances'][:]
        offset = sample % 3
        order = order[offset:] + order[:offset]
        for allowance in order:
            name = f'{"warmup" if sample == -1 else f"sample-{sample:02d}"}-{allowance}'
            c, out, err = invoke(name, [binary, 'measure', allowance])
            assert c['exit_code'] == 0 and not c['timed_out'] and not err, (name, err)
            result = json.loads(out)
            assert result['passed'] and result['peer_digest'] == 5468279037442777099
            assert result['victim_freed_bytes'] == 8192 and result['victim_drops'] == 1
            assert result['victim_drops_during_poll'] == 0 and result['no_post_stop_resume']
            observations.append({'command': name, 'sample': sample, 'warmup': sample == -1, **result})
except Exception as error:
    errors.append(repr(error))

measured = [r for r in observations if r['kind'] == 'measurement' and not r['warmup']]
groups = []
for allowance in protocol['allowances']:
    rows = [r for r in measured if r['allowance'] == allowance]
    if not rows: continue
    metrics = {}
    for metric in protocol['metrics']:
        values = [r[metric] for r in rows]
        metrics[metric] = {'min': min(values), 'median': statistics.median(values), 'max': max(values)}
    rates = [8192 * 1e9 / r['peer_completion_ns'] for r in rows]
    metrics['peer_words_per_second'] = {'min': min(rates), 'median': statistics.median(rates), 'max': max(rates)}
    groups.append({'allowance': allowance, 'samples': len(rows), 'metrics': metrics})
comparisons = []
if len(measured) == 27:
    lookup = {(r['sample'], r['allowance']): r for r in measured}
    for allowance in [32, 256]:
        ratios = [lookup[(s, 1)]['peer_completion_ns']/lookup[(s, allowance)]['peer_completion_ns'] for s in range(9)]
        comparisons.append({'baseline_allowance': 1, 'treatment_allowance': allowance,
                            'paired_peer_throughput_ratios': ratios, 'median': statistics.median(ratios)})
result = {
    'status': 'failed' if errors else 'passed', 'errors': errors, 'protocol': record(RUN / 'protocol.json'),
    'inputs': inputs, 'tools': [record(TOOLCHAIN / t) for t in ['rustc', 'cargo']], 'artifacts': artifacts,
    'commands': commands, 'observations': observations, 'groups': groups, 'comparisons': comparisons,
    'host': {'platform': platform.platform(), 'cpu_count': os.cpu_count(), 'affinity': AFFINITY,
             'load_after': Path('/proc/loadavg').read_text().strip()},
    'environment_overrides': OVERRIDES, 'new_beam_execution': False, 'production_runtime': False,
}
(RUN / 'results.json').write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps({'status': result['status'], 'run': str(RUN), 'commands': len(commands),
                  'measured': len(measured), 'groups': groups, 'errors': errors}, indent=2))
raise SystemExit(bool(errors))
