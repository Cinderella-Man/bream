"""Run bounded Linux contract probes, recording inputs and raw outputs."""
from pathlib import Path
import hashlib
import json
import os
import platform
import shutil
import signal
import subprocess

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[1]
BIN = ROOT / '.tools/io-lifetime-review'
BIN.mkdir(parents=True, exist_ok=True)
RUNS = BASE / 'runs'
RUNS.mkdir(exist_ok=True)
EXPECTED = {
    'copied_event_reuse': {'case': 'copied_event_reuse', 'old_token': 101, 'new_token': 202,
                           'same_fd_reused': True, 'read_from_reused_fd': 'b'},
    'duplicate_lifetime': {'case': 'duplicate_lifetime', 'event_after_original_close': True,
                           'delete_via_duplicate': 'ENOENT', 'empty_after_last_close': True},
    'consumed_readiness': {'case': 'consumed_readiness', 'readiness_observed': True,
                           'second_read': 'EAGAIN'},
    'oneshot_rearm': {'case': 'oneshot_rearm', 'disabled_before_mod': True,
                      'ready_after_mod': True, 'rearmed_token': 506},
    'hup_with_data': {'case': 'hup_with_data', 'readable_and_hup': True,
                      'data_before_eof': True},
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return str(path.relative_to(ROOT))


def run(argv, stem, timeout):
    proc = subprocess.Popen(argv, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            start_new_session=True)
    timed_out = False
    try:
        out, err = proc.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(proc.pid, signal.SIGKILL)
        out, err = proc.communicate()
    result = {'argv': argv, 'cwd': str(ROOT), 'exit_code': proc.returncode,
              'timeout_seconds': timeout, 'timed_out': timed_out}
    for name, data in [('stdout', out), ('stderr', err)]:
        path = RUNS / f'{stem}.{name}'
        path.write_bytes(data)
        result[name] = relative(path)
        result[name + '_sha256'] = digest(path)
    return result, out


compiler = Path(shutil.which('cc')).resolve()
version, version_out = run([str(compiler), '--version'], 'compiler-version', 5)
source = BASE / 'epoll_contract.c'
result = {
    'scope': 'Sequential Linux epoll contract observations, not BEAM/Bream, concurrency or performance qualification.',
    'platform': platform.uname()._asdict(),
    'compiler': {'path': str(compiler), 'sha256': digest(compiler),
                 'version': version_out.decode(), 'command': version},
    'runner': {'path': relative(Path(__file__)), 'sha256': digest(Path(__file__))},
    'source': {'path': relative(source), 'sha256': digest(source)},
    'expected': EXPECTED, 'profiles': [],
}
success = version['exit_code'] == 0 and not version['timed_out']
for profile, opt in [('unoptimized', '-O0'), ('optimized', '-O2')]:
    binary = BIN / profile
    compile_result, _ = run([str(compiler), '-std=c11', '-Wall', '-Wextra', '-Werror',
                             opt, '-g', str(source), '-o', str(binary)], profile + '-compile', 30)
    row = {'name': profile, 'compile': compile_result, 'executions': []}
    compiled = compile_result['exit_code'] == 0 and not compile_result['timed_out']
    success &= compiled
    if compiled:
        row['artifact'] = {'path': relative(binary), 'sha256': digest(binary)}
        for case, expected in EXPECTED.items():
            command, out = run([str(binary), case], profile + '-' + case, 5)
            try:
                observed = json.loads(out)
            except (ValueError, UnicodeError):
                observed = None
            matched = (command['exit_code'] == 0 and not command['timed_out']
                       and observed == expected and (ROOT / command['stderr']).stat().st_size == 0)
            success &= matched
            row['executions'].append({'case': case, 'command': command,
                                      'observed': observed, 'expected_observation': matched})
    result['profiles'].append(row)
result['compile_invocations'] = len(result['profiles'])
result['execution_invocations'] = sum(len(p['executions']) for p in result['profiles'])
result['all_expected_observations'] = bool(success)
(BASE / 'probe-results.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({k: result[k] for k in ['compile_invocations', 'execution_invocations',
                                        'all_expected_observations']}, indent=2))
raise SystemExit(not success)
