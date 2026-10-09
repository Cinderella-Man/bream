#!/usr/bin/env python3
"""Archived Rust + modern ownership probes. Correctness only; no measurements."""
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
MODERN = Path('/home/bat/projects/rust-actor-runtime-lab/.tools/rustup/toolchains/1.98.1-x86_64-unknown-linux-gnu/bin/rustc')
HISTORICAL = {
    'no_yield_gate': {'peer_saw_finished_phase': True},
    'cooperative_yield': {'peer_saw_active_phase': True, 'stack_preserved': True},
    'channel_and_task_local': {'child_ran': True, 'locals_isolated': True},
    'panic_cleanup': {'drops': 1, 'peer_reply': 17},
}
MODERN_CASES = {
    'owned_cancel': {'actor_drops': 1, 'buffer_retained_after_stop': True,
                     'peer_before_completion': 17, 'buffer_drops': 1},
    'owned_complete': {'value': 7, 'actor_drops': 1, 'buffer_drops': 1},
    'reused_identity': {'stale_rejected': True, 'replacement_value': 99, 'buffer_drops': 1},
    'wake_registration': {'schedules': 3, 'naive_violations': 1,
                          'barrier_violations': 0, 'consuming_registration': True},
}
REJECTIONS = {'reject_borrowed_drop.rs': 'E0505', 'reject_registration_reuse.rs': 'E0382'}


def sha(path):
    return hashlib.file_digest(Path(path).open('rb'), 'sha256').hexdigest()


def relative(path):
    return str(Path(path).relative_to(ROOT))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--label', required=True)
    args = parser.parse_args()
    if not args.label or any(c not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_' for c in args.label):
        parser.error('label must contain only letters, numbers, hyphens or underscores')
    run = ROOT / 'evidence/rust-green-history/runs' / args.label
    run.mkdir(parents=True, exist_ok=False)
    build = ROOT / '.tools/rust-green-history/runs' / args.label
    build.mkdir(parents=True, exist_ok=False)
    inputs = run / 'inputs'
    inputs.mkdir()
    names = ['historical.rs', 'modern.rs', 'registration.rs', *REJECTIONS, 'run.py', 'prepare.py']
    source_rows = []
    for name in names:
        source, target = HERE / name, inputs / name
        shutil.copyfile(source, target)
        source_rows.append({'source': relative(source), 'retained': relative(target), 'sha256': sha(target)})
    provenance_path = ROOT / 'evidence/rust-green-history/provenance.json'
    shutil.copyfile(provenance_path, inputs / 'provenance.json')
    provenance = json.loads(provenance_path.read_text())
    archive = provenance['official_compiler']
    old_root = ROOT / archive['unpacked_path']
    old_compiler = old_root / 'bin/rustc'
    record = {
        'schema': 1,
        'recorded_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'scope': 'historical cooperative scheduling and modern lifetime/identity correctness; no performance evidence',
        'source_inputs': source_rows,
        'provenance': {'retained': relative(inputs / 'provenance.json'), 'sha256': sha(inputs / 'provenance.json')},
        'historical_source_manifests': [], 'tools': [], 'commands': [], 'observations': [],
        'all_expected_observations': False,
        'limits': [
            'Historical execution is Rust 0.12 at ba4081a5a, not the earlier 2013 linked-failure runtime.',
            'The historical no-yield gate has an external releaser; it does not run an intentionally endless task.',
            'The modern owned-operation driver is a safe, channel-gated stand-in; no native I/O cancellation or FFI is exercised.',
            'Modern peer progress is a separate application step in the probe, not an integration result from the Bream scheduler.',
            'Registration enumeration covers three abstract event orders, not all memory-model interleavings.',
            'No arbitrary-PC interruption, stack abandonment, forced thread kill, or production-runtime modification.',
            'Watchdog deadlines detect hangs only; no durations, throughput, latency, or rankings are recorded.',
        ],
    }
    env = os.environ.copy()
    env['LD_LIBRARY_PATH'] = str(old_root / 'lib')
    env['RUST_BACKTRACE'] = '0'

    def persist():
        (run / 'results.json').write_text(json.dumps(record, indent=2) + '\n')

    def invoke(name, command, reject_code=None, timeout=30):
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
        expected_exit = process.returncode == 0 if reject_code is None else process.returncode == 1 and f'error[{reject_code}]' in stderr
        row = {'name': name, 'argv': command, 'exit_code': process.returncode,
               'timed_out': timed_out, 'watchdog_seconds': timeout, 'expected_rejection_code': reject_code,
               'stdout': relative(out), 'stdout_sha256': sha(out),
               'stderr': relative(err), 'stderr_sha256': sha(err),
               'matched_expectation': expected_exit and not timed_out}
        record['commands'].append(row)
        persist()
        assert row['matched_expectation'], (name, row)
        return stdout

    try:
        assert sha(ROOT / archive['archive_path']) == archive['archive_sha256']
        record['archive_checksum_verified'] = True
        for row in provenance['source_snapshots']:
            manifest = ROOT / row['manifest']
            source = json.loads(manifest.read_text())
            for item in source['files']:
                assert sha(ROOT / row['checkout'] / item['path']) == item['sha256'], item['path']
            saved = inputs / Path(row['manifest']).name
            shutil.copyfile(manifest, saved)
            record['historical_source_manifests'].append({**row, 'retained': relative(saved), 'sha256': sha(saved), 'files_checked': len(source['files'])})
        artifacts = [old_compiler, *sorted((old_root / 'lib').glob('*.so')),
                     *sorted((old_root / 'lib/rustlib/x86_64-unknown-linux-gnu/lib').glob('*.rlib'))]
        record['historical_artifacts'] = [{'path': relative(p), 'sha256': sha(p)} for p in artifacts]
        for name, compiler, options in [('historical', old_compiler, ['--version']),
                                        ('modern', MODERN, ['--version', '--verbose'])]:
            version = invoke(name + '-version', [compiler, *options])
            if name == 'historical':
                assert 'rustc 0.12.0 (ba4081a5a' in version, version
            record['tools'].append({'name': name, 'path': str(compiler), 'sha256': sha(compiler), 'version': version})
        for profile, level in [('unoptimized', '0'), ('optimized', '3')]:
            for baseline, compiler, fixture, cases in [
                ('historical', old_compiler, 'historical.rs', HISTORICAL),
                ('modern', MODERN, 'modern.rs', MODERN_CASES),
            ]:
                binary = build / (baseline + '-' + profile)
                flags = [] if baseline == 'historical' else ['--edition=2024', '-C', 'panic=unwind']
                invoke(baseline + '-' + profile + '-compile', [compiler, *flags,
                       '-D', 'warnings', '-C', 'opt-level=' + level, inputs / fixture, '-o', binary], timeout=60)
                for case, values in cases.items():
                    expected = {'case': case, **values}
                    actual = json.loads(invoke(baseline + '-' + profile + '-' + case, [binary, case]))
                    record['observations'].append({'baseline': baseline, 'profile': profile, 'case': case,
                        'expected': expected, 'actual': actual, 'matches': actual == expected,
                        'binary': relative(binary), 'binary_sha256': sha(binary)})
                    persist()
                    assert actual == expected, (case, actual, expected)
            for fixture, code in REJECTIONS.items():
                name = profile + '-' + fixture[:-3]
                invoke(name, [MODERN, '--edition=2024', '-C', 'opt-level=' + level,
                             inputs / fixture, '-o', build / name], reject_code=code, timeout=60)
                record['observations'].append({'baseline': 'compile-rejection', 'profile': profile,
                                              'case': fixture, 'expected_code': code, 'matches': True})
        assert len(record['commands']) == 26
        assert len(record['observations']) == 20
        record['all_expected_observations'] = True
        persist()
        print(json.dumps({'run': relative(run), 'expected_commands': 26, 'observations': 20, 'passed': True}))
    finally:
        persist()


if __name__ == '__main__':
    main()
