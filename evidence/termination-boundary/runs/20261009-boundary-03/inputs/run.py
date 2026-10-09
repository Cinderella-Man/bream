#!/usr/bin/env python3
"""Correctness only: isolated, channel-gated Rust and pinned BEAM probes.

Watchdogs detect hangs. This runner records no durations and has no benchmark
phase. Retained inputs and exact observation oracles make each run auditable.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
RUSTC = Path('/home/bat/projects/rust-actor-runtime-lab/.tools/rustup/toolchains/1.98.1-x86_64-unknown-linux-gnu/bin/rustc')
OTP = ROOT / 'upstream/otp-reference-build'
OTP_COMMIT = 'ad05823719d77c8faee87348ea39513d4e2f99c5'
RUST_EXPECTED = {
    'owned_cancel': {'peer_total': 200, 'frame_drops': 1},
    'shared_cancel': {'peer_total': 199, 'left': 99, 'right': 100,
                      'mutex_unpoisoned': 1, 'frame_drops': 1},
    'shared_complete': {'peer_total': 200, 'left': 99, 'right': 101,
                        'mutex_unpoisoned': 1, 'frame_drops': 1},
    'atomic_commit_cancel': {'peer_total': 200, 'left': 99, 'right': 101,
                             'mutex_unpoisoned': 1, 'frame_drops': 1},
    'rollback_cancel': {'peer_total': 200, 'rollbacks': 1},
    'child_survives': {'parent_drops': 1, 'child_effects_after_parent_drop': 1,
                       'child_drops_after_explicit_stop': 1},
    'managed_child_cancel': {'child_effects': 0, 'child_drops': 1,
                             'post_drop_command_rejected': 1},
    'blocking_drop': {'peer_blocked_at_destructor_gate': 1, 'cleanup_finished': 1,
                      'peer_reply': 17},
    'bounded_drop': {'peer_blocked_at_destructor_gate': 0, 'cleanup_finished': 1,
                     'peer_reply': 17},
}
BEAM_EXPECTED = [
    {'case': 'owned_kill', 'peer_total': 200},
    {'case': 'ets_cancel', 'peer_total': 199},
    {'case': 'ets_complete', 'peer_total': 200},
    {'case': 'ets_atomic_cancel', 'peer_total': 200},
    {'case': 'unlinked_child', 'child_effects_after_parent_down': 1},
    {'case': 'linked_child', 'child_effects_after_parent_down': 0},
]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def relative(path):
    return str(Path(path).relative_to(ROOT))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--label', required=True)
    args = parser.parse_args()
    if not args.label or any(c not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_' for c in args.label):
        parser.error('label must contain only letters, numbers, hyphens or underscores')
    run = ROOT / 'evidence/termination-boundary/runs' / args.label
    run.mkdir(parents=True, exist_ok=False)
    build = ROOT / '.tools/termination-boundary' / args.label
    build.mkdir(parents=True, exist_ok=False)
    inputs = run / 'inputs'
    inputs.mkdir()
    source_rows = []
    for name in ['Cargo.toml', 'src/main.rs', 'termination_reference.erl', 'run.py']:
        source, saved = HERE / name, inputs / name
        saved.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, saved)
        source_rows.append({'source': relative(source), 'retained': relative(saved),
                            'sha256': sha(saved)})
    record = {
        'schema': 1,
        'recorded_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'scope': 'bounded cancellation/ownership/peer semantic probes; no timing or performance comparisons',
        'source_inputs': source_rows,
        'commands': [],
        'observations': [],
        'all_expected_observations': False,
        'limits': [
            'Rust probes use standard-library futures and explicit test gates, not the checked Bream API.',
            'Rust cancellation happens only after poll returns Pending; no live native stack is abandoned.',
            'The original native preemption experiment is not rerun and arbitrary native preemption is not qualified.',
            'Shared-state cancellation demonstrates lost application invariants, not memory unsafety.',
            'Explicit rollback and joining are controls for these bounded protocols, not universal cancellation solutions.',
            'BEAM controls do not exercise NIFs, speed, scheduler latency, or full resource cleanup.',
            'External watchdogs only detect hangs; their values and elapsed time are not performance evidence.',
        ],
    }
    env = os.environ.copy()
    env.update({'ERL_FLAGS': '+S 1:1 +SDcpu 1 +SDio 1', 'ERL_CRASH_DUMP': str(build / 'erl_crash.dump')})

    def persist():
        (run / 'results.json').write_text(json.dumps(record, indent=2) + '\n')

    def invoke(name, command, timeout=30):
        command = [str(x) for x in command]
        process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, text=True, start_new_session=True)
        timed_out = False
        try:
            stdout, stderr = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGKILL)
            stdout, stderr = process.communicate()
        out, err = run / (name + '.stdout'), run / (name + '.stderr')
        out.write_text(stdout)
        err.write_text(stderr)
        row = {'name': name, 'argv': command, 'exit_code': process.returncode,
               'timed_out': timed_out, 'watchdog_seconds': timeout,
               'stdout': relative(out), 'stdout_sha256': sha(out),
               'stderr': relative(err), 'stderr_sha256': sha(err),
               'succeeded': process.returncode == 0 and not timed_out}
        record['commands'].append(row)
        persist()
        if not row['succeeded']:
            raise RuntimeError(f'{name} failed; inspect {err}')
        return stdout

    try:
        record['rustc'] = {'path': str(RUSTC), 'sha256': sha(RUSTC),
                            'version': invoke('rustc-version', [RUSTC, '--version', '--verbose'])}
        commit = invoke('otp-commit', ['git', '-C', ROOT / 'upstream/otp', 'rev-parse', 'HEAD']).strip()
        assert commit == OTP_COMMIT, commit
        build_commit = invoke('otp-build-commit', ['git', '-C', OTP, 'rev-parse', 'HEAD']).strip()
        assert build_commit == OTP_COMMIT, build_commit
        provenance_path = ROOT / 'evidence/process-protocol-review/source-build.json'
        provenance = json.loads(provenance_path.read_text())
        assert provenance['source_commit'] == OTP_COMMIT
        assert provenance['source_release_file'] == (OTP / 'OTP_VERSION').read_text().strip() == '29.1.1'
        for artifact in provenance['binary_hashes']:
            assert sha(ROOT / artifact['path']) == artifact['sha256'], artifact['path']
        saved_provenance = inputs / 'otp-source-build.json'
        shutil.copyfile(provenance_path, saved_provenance)
        record['otp'] = {'commit': commit, 'erl_path': str(OTP / 'bin/erl'),
                         'erl_sha256': sha(OTP / 'bin/erl'),
                         'erlc_sha256': sha(OTP / 'bin/erlc'),
                         'build_commit': build_commit,
                         'prior_build_provenance': relative(provenance_path),
                         'retained_build_provenance': relative(saved_provenance),
                         'provenance_sha256': sha(saved_provenance),
                         'prior_binary_hashes_match': True,
                         'environment': {'ERL_FLAGS': env['ERL_FLAGS']}}
        emulators = sorted((OTP / 'bin/x86_64-pc-linux-gnu').glob('beam*'))
        record['otp']['emulators'] = [
            {'path': str(p), 'sha256': sha(p)} for p in emulators if p.is_file()
        ]
        assert record['otp']['emulators'], 'no reference emulator binaries found'
        for profile, level in [('unoptimized', '0'), ('optimized', '3')]:
            binary = build / profile
            invoke(profile + '-compile', [RUSTC, '--edition=2024', '-Dwarnings',
                   '-C', 'opt-level=' + level, '-C', 'panic=unwind', inputs / 'src/main.rs',
                   '-o', binary], timeout=60)
            for case, expected in RUST_EXPECTED.items():
                actual = json.loads(invoke(profile + '-' + case, [binary, case]))
                expected = {'case': case, **expected}
                matches = actual == expected
                record['observations'].append({'language': 'rust', 'profile': profile,
                    'case': case, 'actual': actual, 'expected': expected,
                    'matches': matches, 'binary': relative(binary), 'binary_sha256': sha(binary)})
                persist()
                assert matches, (case, actual, expected)
        invoke('beam-compile', [OTP / 'bin/erlc', '-Werror', '+debug_info', '-o', build,
                               inputs / 'termination_reference.erl'], timeout=60)
        for profile, flags, flavor in [('jit', [], 'jit'),
                                      ('debug-emu', ['-emu_type', 'debug', '-emu_flavor', 'emu'], 'emu')]:
            actual = json.loads(invoke('beam-' + profile, [OTP / 'bin/erl', *flags,
                '-noshell', '-pa', build, '-s', 'termination_reference', 'run']))
            assert actual['otp_release'] == '29', actual
            assert actual['emu_flavor'] == flavor, actual
            assert len(actual['cases']) == len(BEAM_EXPECTED), actual
            for observed, expected in zip(actual['cases'], BEAM_EXPECTED):
                matches = observed == expected
                record['observations'].append({'language': 'erlang', 'profile': profile,
                    'case': expected['case'], 'actual': observed, 'expected': expected,
                    'matches': matches})
                assert matches, (observed, expected)
            record['otp'][profile] = {k: v for k, v in actual.items() if k != 'cases'}
            record['otp']['fixture_sha256'] = sha(build / 'termination_reference.beam')
            persist()
        assert len(record['observations']) == 30
        record['all_expected_observations'] = all(row['matches'] for row in record['observations'])
    except Exception as error:
        record['error'] = repr(error)
    persist()
    summary = {'result': relative(run / 'results.json'),
               'commands': len(record['commands']),
               'observations': len(record['observations']),
               'all_expected_observations': record['all_expected_observations']}
    if 'error' in record:
        summary['error'] = record['error']
    print(json.dumps(summary, indent=2))
    return 0 if record['all_expected_observations'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
