#!/usr/bin/env python3
"""Audit retained correctness evidence. Never builds or executes experiments."""
from pathlib import Path
import collections
import hashlib
import json
import subprocess
import oracles

ROOT = Path(__file__).resolve().parents[2]
BASE = Path(__file__).resolve().parent
errors = []
checked = 0


def check(condition, message):
    if not condition:
        errors.append(message)


def read(path):
    return json.loads(path.read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(record):
    global checked
    path = ROOT / record['path']
    check(path.is_file(), 'Missing ' + record['path'])
    if path.is_file():
        check(digest(path) == record['sha256'], 'Hash differs ' + record['path'])
        checked += 1


def records(value):
    if isinstance(value, dict):
        if isinstance(value.get('path'), str) and isinstance(value.get('sha256'), str):
            yield value
        for child in value.values():
            yield from records(child)
    elif isinstance(value, list):
        for child in value:
            yield from records(child)


runs = sorted((BASE / 'runs').glob('*/results.json'))
assert runs, 'No retained run'
latest = runs[-1]
result = read(latest)
for path in runs:
    for record in records(read(path)):
        verify(record)
for item in result['inputs']:
    check(digest(ROOT / item['original_path']) == item['sha256'], 'Current input differs: ' + item['original_path'])
check(result['status'] == 'passed' and not result['errors'], 'Final cohort passed')
check(result['correctness_only'] and result['speed_tests'] is False and result['performance_comparisons'] is False, 'Correctness-only scope')
commands = {r['name']: r for r in result['commands']}
check(len(commands) == len(result['commands']) == 454, '454 distinct expected commands')
check(len(result['observations']) == 408, '408 observations')
for name, row in commands.items():
    check(not row['timed_out'], 'No watchdog: ' + name)
    check((row['exit_code'] != 0) == name.startswith('reject-'), 'Expected exit status: ' + name)
for profile in ['debug', 'release']:
    check('119 passed; 0 failed' in (ROOT / commands['unit-' + profile]['stdout']['path']).read_text(), 'Runtime test total')
    rejections = [r for r in result['observations'] if r.get('kind') == 'rejection' and r['profile'] == profile]
    check(len(rejections) == 169, '169 rejections per profile')
    for row in rejections:
        err = (ROOT / commands[row['command']]['stderr']['path']).read_text()
        check(row['diagnostic'] in err, 'Required rejection: ' + row['fixture'])
    for example in ['shadow_paths', 'shadow_structured', 'shadow_mailbox', 'shadow_timeout', 'shadow_monitor', 'shadow_link', 'shadow_spawn', 'shadow_shared', 'shadow_exit', 'shadow_read']:
        row = commands[example + '-execute-' + profile]
        check((ROOT / row['stdout']['path']).read_text() == example.upper() + '_PASSED\n', example)
        check(not (ROOT / row['stderr']['path']).read_text(), 'Clean shadow stderr')
    for kind in ['controls', 'timeouts', 'monitors', 'links', 'spawns', 'room', 'shared', 'busy', 'replacement', 'exits', 'reads']:
        for allowance in [1, 4, 32]:
            row = commands[f'{kind}-{profile}-{allowance}']
            output = read(ROOT / row['stdout']['path'])
            check(output['passed'] and output['speed_tests'] is False, 'Control result: ' + row['name'])
exit_rows = [r for r in result['observations'] if r.get('kind') == 'exit_controls']
check({(r['profile'], r['allowance']) for r in exit_rows} == {(p, a) for p in ['debug', 'release'] for a in [1, 4, 32]}, 'Six new configurations')
for row in exit_rows:
    try:
        oracles.check_exit(row)
    except AssertionError:
        errors.append('Exit semantic oracle differs: ' + row['command'])
    actual = read(ROOT / commands[row['command']]['stdout']['path'])
    check(all(actual.get(k) == v for k, v in row.items() if k not in ['command', 'profile']), 'Retained exit output agrees')
beam_rows = [r for r in result['observations'] if r.get('kind') == 'beam_reference']
check({r['profile'] for r in beam_rows} == {'jit', 'debug-emu'}, 'Two emulator profiles')
for row in beam_rows:
    check(row['cases'] == oracles.reference(), '39 exact BEAM rows')
    check(row['otp_release'] == '29', 'OTP release')
    actual = read(ROOT / commands[row['command']]['stdout']['path'])
    check(actual['cases'] == row['cases'], 'Retained BEAM output agrees')
    matrix = [[r[k] for k in ['mode', 'reason', 'trap', 'outcome', 'observed_reason', 'returned']] for r in row['cases'] if r['kind'] == 'matrix']
    check(all(r['matrix'] == matrix for r in exit_rows), 'Public Rust matrix matches BEAM')
read_rows = [r for r in result['observations'] if r.get('kind') == 'read_controls']
check({(r['profile'],r['allowance']) for r in read_rows} == {(p,a) for p in ['debug','release'] for a in [1,4,32]}, 'Six read configurations')
for row in read_rows:
    try:
        oracles.check_read(row)
    except AssertionError:
        errors.append('Read semantic oracle differs: ' + row['command'])
    actual = read(ROOT / commands[row['command']]['stdout']['path'])
    check(all(actual.get(k) == v for k,v in row.items() if k not in ['command','profile']), 'Retained read output agrees')
# The existing OTP build must still match its retained provenance.
provenance_record = next(r for r in result['inputs'] if r['original_path'] == 'evidence/process-protocol-review/source-build.json')
provenance = read(ROOT / provenance_record['path'])
check(provenance['source_commit'] == 'ad05823719d77c8faee87348ea39513d4e2f99c5', 'Pinned OTP commit')
for row in provenance['binary_hashes']:
    verify(row)
# This milestone must preserve the previous implementation/evidence inputs.
previous = read(ROOT / 'evidence/explicit-exit/runs/20261009-exit-01/results.json')
for row in previous['inputs']:
    check(digest(ROOT / row['original_path']) == row['sha256'], 'Previous milestone preserved: ' + row['original_path'])
report = {'status': 'failed' if errors else 'passed', 'date': '2026-10-09',
          'cohort': str(latest.parent.relative_to(ROOT)), 'retained_cohorts': len(runs),
          'hash_records_verified': checked, 'current_inputs_verified': len(result['inputs']),
          'previous_inputs_preserved': len(previous['inputs']), 'commands': len(commands),
          'observations': len(result['observations']), 'rust_exit_configurations': len(exit_rows), 'rust_read_configurations': len(read_rows),
          'beam_reference_observations': sum(len(r['cases']) for r in beam_rows),
          'experiments_executed_by_audit': False, 'benchmarks_executed': False, 'errors': errors}
(BASE / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
raise SystemExit(bool(errors))
