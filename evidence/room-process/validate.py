#!/usr/bin/env python3
"""Read-only room/session evidence audit; never invokes builds or runtime fixtures."""
from pathlib import Path
import hashlib
import json
import re
import subprocess

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
    check(path.is_file(), 'Missing '+record['path'])
    if path.is_file():
        check(digest(path) == record['sha256'], 'Hash differs '+record['path'])
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
check(bool(runs), 'No retained correctness cohorts')
latest = runs[-1]
result = read(latest)
for path in runs:
    for record in records(read(path)):
        verify(record)
for item in result['inputs']:
    check(digest(ROOT / item['original_path']) == item['sha256'], 'Current source differs '+item['original_path'])
check(result['status'] == 'passed' and not result['errors'], 'Final cohort failed')
check(result['correctness_only'] and result['speed_tests'] is False and result['performance_comparisons'] is False,
      'Invalid correctness-only declaration')
commands = {item['name']: item for item in result['commands']}
check(len(commands) == len(result['commands']) == 317, 'Expected 317 distinct commands')
check(len(result['observations']) == 286, 'Expected 286 observations')
for name, command in commands.items():
    check(not command['timed_out'], 'Watchdog fired '+name)
    check((command['exit_code'] != 0) == name.startswith('reject-'), 'Unexpected exit '+name)
    check(not {'measure','benchmark','churn','lookup'}.intersection(command['argv']), 'Speed command '+name)
for observation in result['observations']:
    command = commands[observation['command']]
    stdout = (ROOT / command['stdout']['path']).read_text()
    stderr = (ROOT / command['stderr']['path']).read_text()
    check(observation['passed'], 'Failed observation '+command['name'])
    if observation['kind'] == 'unit_tests':
        check(observation['tests'] == 83 and '83 passed; 0 failed' in stdout, 'Unit record '+command['name'])
    elif observation['kind'] == 'rejection':
        check(observation['diagnostic'] in stderr, 'Wrong rejection '+command['name'])
    else:
        raw = json.loads(stdout)
        check({k:v for k,v in observation.items() if k not in ('command','profile')} == raw,
              'Output/observation mismatch '+command['name'])
for row in result['observations']:
    if row['kind'] == 'room_controls':
        expected_digest = 0
        by_count = {}
        for count, event in enumerate(row['expected_transcript'],1):
            for value in event:
                expected_digest = (expected_digest*131+value) % (1<<64)
            by_count[count] = (expected_digest,event[9])
        check(row['observed_records'] == len(row['expected_transcript']) == 15, 'Room transcript count')
        check(row['observed_digest'] == expected_digest, 'Room transcript digest')
        check([value[0] for value in row['observed_checkpoints']] == [1,2,3,4,5,6,7,8,10,11,12,13,14,15], 'Room barrier counts')
        for count, observed_digest, sequence in row['observed_checkpoints']:
            check((observed_digest,sequence) == by_count[count], 'Room barrier observation')
for profile in ('debug','release'):
    for example in ('shadow_paths','shadow_structured','shadow_mailbox','shadow_timeout','shadow_monitor','shadow_link','shadow_spawn'):
        command = commands[example+'-execute-'+profile]
        check((ROOT / command['stdout']['path']).read_text() == example.upper()+'_PASSED\n', 'Positive shadow '+example)
    for allowance in (1,4,32):
        check(f'room-{profile}-{allowance}' in commands, 'Missing room configuration')
prior_hashes = 0
for prior in sorted((ROOT/'evidence/spawn-process/runs').glob('*/results.json')):
    for record in records(read(prior)):
        verify(record)
        prior_hashes += 1
prior = read(ROOT/'evidence/spawn-process/runs/20261008-spawn-02/results.json')
for item in prior['inputs']:
    check(digest(ROOT/item['original_path']) == item['sha256'], 'Earlier spawn source changed '+item['original_path'])
# This integration slice adds application code; runtime/compiler semantics are inherited.
inherited = 0
for group in ('runtime', 'macros'):
    for path in sorted((ROOT/'experiments/room-process'/group).rglob('*')):
        if path.is_file() and path.suffix in ('.rs','.toml'):
            prior_path = ROOT/'experiments/spawn-process'/path.relative_to(ROOT/'experiments/room-process')
            expected = prior_path.read_text().replace('bream-spawn','bream-room').replace('bream_spawn','bream_room')
            check(path.read_text() == expected, 'Unexpected runtime/compiler change '+str(path.relative_to(ROOT)))
            inherited += 1
# The existing audit preserves the removal baseline and scans all subsequent
# JSON records. It only reads files and writes its own audit report.
(BASE/'validation.json').write_text(json.dumps({'status':'audit_in_progress','benchmarks_executed':False})+'\n')
audit = subprocess.run(['python3', str(ROOT/'evidence/performance-reset/validate.py')], capture_output=True, text=True)
check(audit.returncode == 0, 'Removal-baseline audit failed: '+audit.stdout+audit.stderr)
source_status = subprocess.run(['git','-C',str(ROOT/'upstream/otp'),'status','--porcelain'],capture_output=True,text=True,check=True)
check(not source_status.stdout, 'Pinned upstream checkout changed')
report = {
    'status':'failed' if errors else 'passed', 'date':'2026-10-08',
    'cohort':str(latest.parent.relative_to(ROOT)), 'retained_cohorts':len(runs),
    'hash_records_verified':checked, 'current_inputs_verified':len(result['inputs']),
    'prior_spawn_hashes_verified':prior_hashes, 'inherited_runtime_compiler_files_verified':inherited,
    'commands':len(commands), 'observations':len(result['observations']),
    'runtime_tests_executed_by_audit':False, 'benchmarks_executed':False,
    'performance_qualification':'prohibited on this machine', 'errors':errors,
}
(BASE/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
raise SystemExit(bool(errors))
