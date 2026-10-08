#!/usr/bin/env python3
"""Audit the post-removal files. Never invoke an experiment or benchmark.

Hashes establish the new baseline, not the authenticity of an untouched original
run. Correctness checks below inspect retained outputs without executing them.
"""
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tarfile
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[2]
BASE = Path(__file__).resolve().parent
errors = []
counts = Counter()


def check(condition, message):
    if not condition:
        errors.append(message)


def read(path):
    return json.loads(path.read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(record):
    path = ROOT / record['path']
    check(path.is_file(), f"Missing file: {record['path']}")
    if path.is_file():
        check(digest(path) == record['sha256'], f"Hash mismatch: {record['path']}")
        counts['hashes_verified'] += 1


forbidden = {
    'elapsed_seconds', 'elapsed_seconds_not_benchmark', 'wall_seconds',
    'total_child_wall_seconds', 'total_child_user_seconds',
    'total_child_system_seconds', 'elapsed_ms', 'harness_elapsed_seconds',
    'resume_read_elapsed_ms', 'suspend_elapsed_ms', 'median', 'measurements',
    'comparisons',
}


def scan(value, location):
    if isinstance(value, dict):
        for key, child in value.items():
            check(key not in forbidden and not key.endswith('_ns'),
                  f"Speed-result field: {location}/{key}")
            if key == 'kind':
                check(child not in ('measurement', 'churn', 'lookup'),
                      f"Speed-result observation: {location}")
            scan(child, location + '/' + key)
    elif isinstance(value, list):
        for i, child in enumerate(value):
            scan(child, f'{location}/{i}')


manifest = read(BASE / 'manifest.json')
receipt = read(BASE / 'removal.json')
for section in ('evidence_files', 'implementation_inputs'):
    for record in manifest[section]:
        verify(record)
    counts[section] = len(manifest[section])
listed = {r['path'] for r in manifest['evidence_files']}
actual = {str(p.relative_to(ROOT)) for p in (ROOT / 'evidence').rglob('*')
          if p.is_file() and BASE not in p.parents and '__pycache__' not in p.parts}
check(listed <= actual, 'Files from the post-removal evidence baseline are missing')
counts['subsequent_evidence_files'] = len(actual - listed)
for name in receipt['deleted_files']:
    check(not (ROOT / name).exists(), f'Deleted speed evidence restored: {name}')
counts['deleted_files_absent'] = len(receipt['deleted_files'])

for name in sorted(actual):
    path = ROOT / name
    if 'inputs' in path.parts:
        continue
    if path.suffix == '.json':
        scan(read(path), name)
        counts['json_records_scanned'] += 1
    elif path.suffix in ('.stdout', '.jsonl'):
        for line in path.read_text().splitlines():
            try:
                value = json.loads(line)
            except ValueError:
                continue
            scan(value, name)
            counts['raw_json_rows_scanned'] += 1

# Latest cohorts and the useful-process predecessor retain passing controls.
cohorts = [
    ('process-milestone', '20261008-useful-process-01', 57, 50),
    ('process-milestone', '20261008-useful-process-02', 57, 50),
    ('structured-process', '20261008-structured-01', 133, 122),
    ('identity-lifecycle', '20261008-identity-01', 145, 134),
    ('mailbox-process', '20261008-mailbox-01', 155, 140),
    ('receive-timeout', '20261008-timeout-04', 183, 164),
]
cohort_summary = []
for group, run, command_count, observation_count in cohorts:
    path = ROOT / 'evidence' / group / 'runs' / run / 'results.json'
    result = read(path)
    commands = {c['name']: c for c in result['commands']}
    check(result['status'] == 'passed' and not result['errors'], f'{run}: status')
    check(len(commands) == len(result['commands']) == command_count, f'{run}: command count')
    check(len(result['observations']) == observation_count, f'{run}: observation count')
    for name, command in commands.items():
        check(not name.startswith(('sample-', 'warmup-', 'registry-', 'churn-', 'lookup-')),
              f'{run}: performance command {name}')
        check(not {'measure', 'churn', 'lookup'}.intersection(command['argv']),
              f'{run}: performance argv {name}')
        check(not command['timed_out'], f'{run}: watchdog fired for {name}')
        check((command['exit_code'] != 0) == name.startswith('reject-'),
              f'{run}: unexpected exit for {name}')
        for stream in ('stdout', 'stderr'):
            verify(command[stream])
    for observation in result['observations']:
        name = observation['command']
        check(observation.get('passed') is True, f'{run}: failed observation {name}')
        command = commands[name]
        stdout = (ROOT / command['stdout']['path']).read_text()
        if observation['kind'] == 'unit_tests':
            check(f"test result: ok. {observation['tests']} passed; 0 failed" in stdout,
                  f'{run}: unit-test record differs from raw output')
        elif observation['kind'] != 'rejection':
            rows = []
            for line in stdout.splitlines():
                try:
                    rows.append(json.loads(line))
                except ValueError:
                    continue
            payload = {k: v for k, v in observation.items() if k not in ('command', 'profile', 'family')}
            check(payload in rows, f'{run}: observation differs from raw output: {name}')
    if group in ('mailbox-process', 'receive-timeout'):
        check(result['correctness_only'] and result['speed_tests'] is False
              and result['performance_comparisons'] is False, f'{run}: correctness-only declaration')
        for record in result['inputs']:
            verify(record)
            check(digest(ROOT / record['original_path']) == record['sha256'],
                  f"{run}: current source differs: {record['original_path']}")
    cohort_summary.append({'run': run, 'commands': command_count, 'observations': observation_count})

for run, status, command_count in [('01', 'failed', 3), ('02', 'passed', 183), ('03', 'failed', 7)]:
    result = read(ROOT / f'evidence/receive-timeout/runs/20261008-timeout-{run}/results.json')
    check(result['status'] == status and len(result['commands']) == command_count,
          f'timeout-{run}: historical correctness outcome changed')

models = read(ROOT / 'evidence/worker-notification-review/results.json')
check(len(models['commands']) == 6 and len(models['models']) == 4, 'Notification model counts')
check(models['all_expected_observations'] and all(m['expected_observation'] for m in models['models']),
      'Notification model outcomes')
check(sum(m['positive'] for m in models['models']) == 2, 'Notification positive/negative controls')

archive_base = ROOT / 'evidence/archive/pre-bream'
archive_records = {r['path']: r['sha256'] for r in read(archive_base / 'snapshot-manifest.json')['snapshot_files']}
with tarfile.open(archive_base / 'project-snapshot.tar.gz', 'r:gz') as archive:
    members = archive.getmembers()
    check({m.name for m in members} == set(archive_records), 'Archive inventory mismatch')
    for member in members:
        path = Path(member.name)
        check(member.isfile() and (path.suffix in ('.rs', '.py', '.toml', '.lock') or path.name == '.gitignore'),
              f'Archive contains non-source file: {member.name}')
        check(hashlib.sha256(archive.extractfile(member).read()).hexdigest() == archive_records[member.name],
              f'Archive hash mismatch: {member.name}')
    counts['archive_source_files'] = len(members)

features = read(ROOT / 'docs/build-plan/features.json')
tasks = read(ROOT / 'docs/build-plan/performance-tasks.json')
check(features['feature_count'] == len(features['features']) == 40 and features['case_count'] == 537,
      'Feature catalogue changed unexpectedly')
check(tasks['task_count'] == len(tasks['tasks']) == 107, 'Performance task count')
check(all(t['status'] == 'planned' and t.get('result') is None for t in tasks['tasks']),
      'Performance tasks contain a retained result')

documents = [ROOT / name for name in ('README.md', 'CONTEXT.md', 'AGENTS.md')]
documents += list((ROOT / 'docs').rglob('*.md'))
documents += [p for base in ('evidence', 'experiments') for p in (ROOT / base).rglob('*.md')
              if not {'inputs', 'vendor', 'target'}.intersection(p.parts)
              and not any(part.startswith('.') for part in p.relative_to(ROOT).parts)]
for path in documents:
    content = path.read_text()
    links = re.findall(r'\[[^\]\n]+\]\(([^\s)]+)', content)
    links += re.findall(r'^\[[^\]]+\]:\s*(\S+)', content, re.MULTILINE)
    for link in links:
        if link.startswith(('#', 'http:', 'https:', 'app:', 'codex:', 'mailto:')):
            continue
        link = re.sub(r':\d+(?:-\d+)?$', '', unquote(link.strip('<>').split('#')[0]))
        target = (path.parent / link).resolve()
        check(target.exists() or target == BASE / 'validation.json', f'Broken link: {path.relative_to(ROOT)} -> {link}')
    counts['documents_checked'] += 1

# Git commands only inspect repository metadata/whitespace; no code is executed.
pin = subprocess.run(['git', '-C', str(ROOT / 'upstream/otp'), 'rev-parse', 'HEAD'],
                     capture_output=True, text=True, check=True).stdout.strip()
check(pin == 'ad05823719d77c8faee87348ea39513d4e2f99c5', 'OTP reference pin changed')
diff = subprocess.run(['git', '-C', str(ROOT), 'diff', '--check'], capture_output=True, text=True)
check(diff.returncode == 0, 'Whitespace check: ' + diff.stdout + diff.stderr)
report = {
    'status': 'passed' if not errors else 'failed',
    'date': '2026-10-08',
    'scope': 'Read-only audit of post-removal evidence; only this audit output is written.',
    'benchmarks_executed': False,
    'runtime_tests_executed': False,
    'performance_status': 'redo_required',
    'counts': dict(counts),
    'retained_correctness_cohorts': cohort_summary,
    'errors': errors,
}
(BASE / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
raise SystemExit(bool(errors))
