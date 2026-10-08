#!/usr/bin/env python3
"""Retain correctness evidence only. This machine must never run speed tests."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
BASE = Path(__file__).resolve().parent
PROJECT = ROOT / 'experiments/receive-timeout'
TOOLCHAIN = Path('/home/bat/projects/rust-actor-runtime-lab/.tools/rustup/toolchains/1.98.1-x86_64-unknown-linux-gnu/bin')
assert len(sys.argv) == 2, 'Supply a unique correctness run name'
RUN = BASE / 'runs' / sys.argv[1]
RUN.mkdir(parents=True, exist_ok=False)
TARGET = ROOT / '.tools/receive-timeout' / RUN.name
OVERRIDES = {'CARGO_HOME': str(ROOT / '.tools/cargo-cache'), 'CARGO_TARGET_DIR': str(TARGET),
             'RUSTC': str(TOOLCHAIN / 'rustc'), 'CARGO_BUILD_JOBS': '2', 'RUSTFLAGS': '', 'RUSTDOCFLAGS': '',
             'RUSTC_WRAPPER': '', 'RUSTC_WORKSPACE_WRAPPER': '', 'RUST_BACKTRACE': '0'}
ENV = os.environ.copy()
ENV.pop('CARGO_ENCODED_RUSTFLAGS', None)
ENV.update(OVERRIDES)
commands, observations, artifacts, errors = [], [], [], []

def record(path):
    path = Path(path)
    return {'path': str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path),
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}

def retained_input(path):
    copy = RUN / 'inputs' / path.relative_to(ROOT)
    copy.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(path, copy)
    return {'original_path': str(path.relative_to(ROOT)), **record(copy)}

def invoke(name, argv, timeout=60):
    child = subprocess.Popen([str(a) for a in argv], cwd=ROOT, env=ENV, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    expired = False
    try:
        out, err = child.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        expired = True
        os.killpg(child.pid, signal.SIGKILL)
        out, err = child.communicate()
    stdout, stderr = RUN / (name+'.stdout'), RUN / (name+'.stderr')
    stdout.write_bytes(out); stderr.write_bytes(err)
    command = {'name': name, 'argv': [str(a) for a in argv], 'exit_code': child.returncode,
               'timed_out': expired, 'watchdog_seconds': timeout, 'stdout': record(stdout), 'stderr': record(stderr)}
    commands.append(command)
    return command, out.decode(), err.decode()

protocol = {
    'date': '2026-10-08', 'milestone': 'Receive timeout scan/wait semantics, cancellation identity and cleanup with injected time',
    'correctness_only': True, 'speed_tests': False, 'performance_comparisons': False,
    'machine_constraint': 'User actively uses this machine for other work. NEVER run speed tests or performance comparisons here; prior local timings are unsuitable performance indicators.',
    'allowances': [1, 4, 32], 'build_jobs': 2,
    'controls': 'Each compiler profile: 35 deterministic protocol/ownership tests; three inherited mailbox controls and three timeout controls; 75 compile-rejected consumers and four positive shadowing routes.',
    'logical_oracles': 'Zero scans before timeout; finite wait arms only after initial scan; unmatched arrivals do not rearm; committed expiry selects timeout before resumed scan; cancelled/detached callback cannot affect another receive or incarnation; exact ownership; max8 expiry callbacks per turn with application work during a19-timer burst.',
    'races': 'Each timeout invocation:16 send/timeout races and16 kill/timeout races. Each inherited mailbox invocation:16 send/kill races and16 concurrent-sender messages. Allowed outcome and ownership assertions only.',
    'timing_policy': 'No sampled timings, rates, warmups, affinity changes or performance summaries. Watchdog timeouts detect hangs only; incidental cargo test/build duration text is not analyzed as evidence.',
    'limits': ['one application worker', 'one concrete application-envelope schema', 'fixed scalar tag or any selector',
               'trusted host allocator and panic behavior', 'logical clock explicitly advanced by host; no real-time timer driver', 'finite delay range0..u32::MAX; no application exception catch', 'no runtime notifications', 'no general worker-failure recovery', 'no full G1/G2/G3 qualification'],
}
(RUN / 'protocol.json').write_text(json.dumps(protocol, indent=2)+'\n')
inputs = [retained_input(p) for p in sorted(PROJECT.rglob('*')) if p.is_file() and p.suffix in ['.rs','.toml','.lock']]
inputs.append(retained_input(Path(__file__).resolve()))
try:
    c, out, err = invoke('rustc-version', [TOOLCHAIN/'rustc', '-vV'])
    assert c['exit_code'] == 0 and 'rustc 1.98.1' in out
    common = ['--offline', '--locked', '--manifest-path', PROJECT/'Cargo.toml']
    build = [TOOLCHAIN/'cargo', 'build', *common, '-p', 'bream-timeout-consumer']
    negatives = sorted((PROJECT/'consumer/examples').glob('reject_*.rs'))
    diagnostic = {
        'reject_future':'mismatched types', 'reject_closure_spawn':'mismatched types',
        'reject_safe_constructor':'requires unsafe function or block', 'reject_program_fields':'is private',
        'reject_drop_input':'mismatched types', 'reject_struct_duplicate_owner':'E0382', 'reject_struct_use_after_move':'E0382',
        'reject_struct_missing_arm':'E0004', 'reject_struct_private_type':'E0603', 'reject_struct_private_helper':'E0603',
        'reject_id_fields':'E0451', 'reject_id_mutation':'E0616', 'reject_send_twice':'E0382', 'reject_message_twice':'E0382',
        'reject_envelope_clone':'E0599', 'reject_send_custom_payload':'E0308',
        'reject_timeout_missing_arm':'E0004', 'reject_timeout_duplicate_owner':'E0382',
    }
    new_grammar = {'reject_receive_predicate','reject_send_numeric_pid','reject_pid_arithmetic','reject_message_constructor','reject_message_builtin_shadow'}
    for profile in ['debug','release']:
        flags = ['--release'] if profile == 'release' else []
        c, out, err = invoke('unit-'+profile, [TOOLCHAIN/'cargo','test',*common,'-p','bream-timeout-runtime','--lib',*flags])
        assert c['exit_code'] == 0 and not c['timed_out'] and '35 passed; 0 failed' in out, (out, err)
        observations.append({'kind':'unit_tests','command':c['name'],'profile':profile,'tests':35,'passed':True})
        c, out, err = invoke('build-'+profile, build+flags)
        assert c['exit_code'] == 0 and not c['timed_out'], err
        binary = TARGET/profile/'bream-timeout-consumer'; artifacts.append(record(binary))
        for allowance in protocol['allowances']:
            c, out, err = invoke(f'controls-{profile}-{allowance}', [binary,'controls',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'] and not err, (out, err)
            row = json.loads(out)
            assert row['passed'] and row['speed_tests'] is False and row['scan_candidates'] == 129
            observations.append({'command':c['name'],'profile':profile,**row})
        for allowance in protocol['allowances']:
            c, out, err = invoke(f'timeouts-{profile}-{allowance}', [binary,'timeouts',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'], (out, err)
            assert 'receive timeout exceeds u32 range' in err and 'receive timer failed: DeadlineOverflow' in err, err
            row = json.loads(out)
            assert row['passed'] and row['speed_tests'] is False and row['burst_service_batches'] == 3
            observations.append({'command':c['name'],'profile':profile,**row})
        for example in ['shadow_paths','shadow_structured','shadow_mailbox','shadow_timeout']:
            c, out, err = invoke(example+'-build-'+profile, build+flags+['--example',example])
            assert c['exit_code'] == 0 and not c['timed_out'], err
            binary = TARGET/profile/'examples'/example; artifacts.append(record(binary))
            c, out, err = invoke(example+'-execute-'+profile, [binary])
            assert c['exit_code'] == 0 and out == example.upper()+'_PASSED\n' and not err, (out, err)
        for fixture in negatives:
            c, out, err = invoke('reject-'+profile+'-'+fixture.stem, build+flags+['--example',fixture.stem])
            default = 'bream structured subset:' if fixture.stem.startswith(('reject_struct_', 'reject_timeout_')) or fixture.stem in new_grammar else 'bream milestone subset:'
            required = diagnostic.get(fixture.stem, default)
            assert c['exit_code'] != 0 and not c['timed_out'] and required in err, (fixture.name, err)
            observations.append({'kind':'rejection','command':c['name'],'profile':profile,'fixture':fixture.stem,'diagnostic':required,'passed':True})
except Exception as error:
    errors.append(repr(error))
result = {
    'status':'failed' if errors else 'passed', 'errors':errors, 'protocol':record(RUN/'protocol.json'),
    'inputs':inputs,'tools':[record(TOOLCHAIN/t) for t in ['rustc','cargo']], 'artifacts':artifacts,
    'commands':commands,'observations':observations,'environment_overrides':OVERRIDES,
    'correctness_only':True, 'speed_tests':False,'performance_comparisons':False,'new_beam_execution':False,'production_runtime':False,
}
(RUN/'results.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'status':result['status'],'commands':len(commands),'observations':len(observations),'inputs':len(inputs),'speed_tests':False,'errors':errors},indent=2))
raise SystemExit(bool(errors))
