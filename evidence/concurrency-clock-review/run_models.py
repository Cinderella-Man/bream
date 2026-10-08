"""Record bounded runs of small Loom projections, including expected failures."""
from pathlib import Path
import hashlib
import json
import os
import platform
import re
import signal
import subprocess

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[1]
PROJECT = ROOT / 'experiments/atomic-protocol-models'
TOOLCHAIN = Path('/home/bat/projects/rust-actor-runtime-lab/.tools/rustup/toolchains/1.98.1-x86_64-unknown-linux-gnu/bin')
TARGET = ROOT / '.tools/atomic-protocol-models'
RUNS = BASE / 'runs'
RUNS.mkdir(parents=True, exist_ok=True)
ENV = dict(os.environ)
for key in list(ENV):
    if key.startswith('LOOM_') or key in ['CARGO_ENCODED_RUSTFLAGS']:
        del ENV[key]
OVERRIDES = {
    'RUSTC': str(TOOLCHAIN / 'rustc'),
    'RUSTFLAGS': '', 'RUSTDOCFLAGS': '', 'RUSTC_WRAPPER': '', 'RUSTC_WORKSPACE_WRAPPER': '',
    'CARGO_HOME': str(ROOT / '.tools/cargo-cache'), 'CARGO_TARGET_DIR': str(TARGET),
    'RUST_BACKTRACE': '0',
}
ENV.update(OVERRIDES)
MODELS = {
    'publication_release_acquire': None,
    'publication_relaxed_store': 'PUBLICATION_STALE',
    'publication_relaxed_load': 'PUBLICATION_STALE',
    'final_owner_acquire_fence': None,
    'final_owner_no_fence': 'LAST_OWNER_STALE',
    'parking_locked': None,
    'parking_separate_acqrel': 'PARKING_LOST_WAKE',
}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def record(path):
    return {'path': str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path),
            'sha256': sha(path)}


def invoke(argv, stem, timeout):
    child = subprocess.Popen([str(v) for v in argv], cwd=ROOT, env=ENV,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    timed_out = False
    try:
        out, err = child.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(child.pid, signal.SIGKILL)
        out, err = child.communicate()
    entry = {'argv': [str(v) for v in argv], 'cwd': str(ROOT), 'exit_code': child.returncode,
             'timeout_seconds': timeout, 'timed_out': timed_out}
    for name, data in [('stdout', out), ('stderr', err)]:
        path = RUNS / f'{stem}.{name}'
        path.write_bytes(data)
        entry[name] = record(path)
    return entry, out.decode(errors='replace'), err.decode(errors='replace')


versions = {}
for name in ['rustc', 'cargo']:
    command, out, err = invoke([TOOLCHAIN / name, '-vV'], name + '-version', 10)
    assert command['exit_code'] == 0 and not command['timed_out'], (out, err)
    versions[name] = record(TOOLCHAIN / name) | {'version': out, 'command': command}

build, out, err = invoke([TOOLCHAIN / 'cargo', 'build', '--release', '--locked',
                         '--manifest-path', PROJECT / 'Cargo.toml', '--message-format=json'], 'build', 180)
result = {
    'scope': 'Three finite Rust synchronization projections, not BEAM/Bream implementation, hardware stress, fairness proof or performance evidence.',
    'platform': platform.uname()._asdict(), 'tools': versions, 'environment_overrides': OVERRIDES,
    'removed_environment': 'All inherited LOOM_* and CARGO_ENCODED_RUSTFLAGS',
    'builder': {'max_threads': 2, 'max_branches': 1000, 'max_permutations': None,
                'max_duration': None, 'preemption_bound': None, 'checkpoint_file': None,
                'location': True, 'log': False},
    'inputs': [record(PROJECT / name) for name in ['Cargo.toml', 'Cargo.lock', 'src/main.rs']],
    'runner': record(Path(__file__)), 'build': build, 'models': [],
}
success = build['exit_code'] == 0 and not build['timed_out']
if success:
    artifacts = [json.loads(line) for line in out.splitlines() if line.startswith('{')]
    binary = next(Path(v['executable']) for v in artifacts
                  if v.get('reason') == 'compiler-artifact' and v.get('executable'))
    result['artifact'] = record(binary)
    for name, marker in MODELS.items():
        command, out, err = invoke([binary, name], name, 45)
        if marker:
            ok = command['exit_code'] == 101 and marker in err and not command['timed_out']
            count = re.search(r'MODEL_EXECUTIONS_STARTED=(\d+)', err)
        else:
            ok = (command['exit_code'] == 0 and not command['timed_out'] and not err
                  and f'model={name} completed=true' in out)
            count = re.search(r'executions_started=(\d+)', out)
        ok = bool(ok and count)
        success &= ok
        result['models'].append({'name': name, 'expected_failure_marker': marker, 'command': command,
                                 'expected_observation': ok,
                                 'executions_started': int(count[1]) if count else None})
result['all_expected_observations'] = bool(success)
(BASE / 'model-results.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'build_ok': build['exit_code'] == 0,
                  'all_expected_observations': bool(success),
                  'models': [{k: row[k] for k in ['name', 'expected_observation', 'executions_started']}
                             for row in result['models']]}, indent=2))
raise SystemExit(not success)
