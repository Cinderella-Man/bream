"""Validate this research pass without rewriting historical evidence."""
from pathlib import Path
import hashlib
import json
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
BASE = Path(__file__).resolve().parent
OTP = ROOT / 'upstream/otp'
PIN = 'ad05823719d77c8faee87348ea39513d4e2f99c5'
errors = []


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check(condition, message):
    if not condition:
        errors.append(message)


docs = [ROOT / 'README.md', ROOT / 'CONTEXT.md']
docs += sorted((ROOT / 'docs').rglob('*.md'))
docs += sorted(BASE.glob('*.md'))
source_paths = set()
source_refs = []
local_links = 0
for path in docs:
    source = path.read_text()
    rel = str(path.relative_to(ROOT))
    check(source.endswith('\n'), f'Missing final newline: {rel}')
    check(all(line == line.rstrip() for line in source.splitlines()),
          f'Trailing whitespace: {rel}')
    definitions = set(x.lower() for x in re.findall(r'^\[([^\]\n]+)\]:', source, re.M))
    for ref in re.findall(r'\[[^\]\n]+\]\[([^\]\n]+)\]', source):
        check(ref.lower() in definitions, f'Undefined reference {ref}: {rel}')
    links = re.findall(r'\[[^\]\n]*\]\((<?[^)\n]+>?)\)', source)
    links += re.findall(r'^\[[^\]\n]+\]:\s*(\S+)', source, re.M)
    for target in links:
        target = target.strip('<>')
        if target.startswith(('https:', 'http:', 'mailto:', '#', 'app:', 'codex:')):
            continue
        target = target.split('#')[0]
        line = None
        match = re.match(r'^(.*):(\d+)$', target)
        if match:
            target, line = match[1], int(match[2])
        linked = Path(target)
        if not linked.is_absolute():
            linked = path.parent / linked
        local_links += 1
        if linked.resolve() == BASE / 'review-validation.json':
            continue  # This run creates its own linked output below.
        check(linked.exists(), f'Missing link {target}: {rel}')
        if line and linked.is_file():
            check(1 <= line <= len(linked.read_text().splitlines()), f'Invalid line {target}:{line}')
    pattern = r'https://github.com/erlang/otp/blob/' + PIN + r'/([^\s)#]+)(?:#L(\d+)(?:-L(\d+))?)?'
    for name, start, end in re.findall(pattern, source):
        pinned = OTP / name
        check(pinned.is_file(), f'Missing pinned source: {name}')
        source_paths.add(name)
        if pinned.is_file() and start:
            check(1 <= int(start) <= int(end or start) <= len(pinned.read_text().splitlines()),
                  f'Invalid source range: {name}:{start}-{end}')
        source_refs.append((rel, name, start, end))

parsed = {}
by_document = {}
case_pattern = r'^\|\s*(?:\*\*)?([A-Z]+-\d{2}\.C\d{2})(?:\*\*)?\s*\|'
for path in sorted((ROOT / 'docs/build-plan').glob('*.md')):
    count = 0
    for match in re.finditer(r'^## ([A-Z]+-\d{2}) — ([^\n]+)\n(.*?)(?=^## |\Z)', path.read_text(), re.M | re.S):
        fid, title, body = match.groups()
        ids = re.findall(case_pattern, body, re.M)
        check(fid not in parsed, 'Duplicate feature: ' + fid)
        check(ids == [fid + f'.C{i:02d}' for i in range(1, len(ids) + 1)], 'Case sequence: ' + fid)
        parsed[fid] = (title, str(path.relative_to(ROOT)), ids)
        count += len(ids)
    by_document[str(path.relative_to(ROOT))] = count

index = json.loads((ROOT / 'docs/build-plan/features.json').read_text())
prior = json.loads((BASE / 'prior-case-ids.json').read_text())
check((prior['feature_count'], prior['case_count']) == (40, 447), 'Prior catalogue mismatch')
for feature in index['features']:
    fid = feature['id']
    title, path, ids = parsed.get(fid, ('', '', []))
    check((feature['title'], feature['detail_document'], feature['case_ids'], feature['case_count']) ==
          (title, path, ids, len(ids)), 'Feature index mismatch: ' + fid)
    check(feature['status'] == 'planned', 'Unexpected qualification: ' + fid)
    old = prior['features'][fid]
    check(ids[:len(old)] == old, 'Prior ID changed/removed: ' + fid)
all_cases = {cid for _, _, ids in parsed.values() for cid in ids}
old_cases = {cid for ids in prior['features'].values() for cid in ids}
check((len(parsed), len(all_cases), index['feature_count'], index['case_count']) == (40, 474, 40, 474),
      'Catalogue total mismatch')
new_cases = {}
reports = []
for number in range(42, 46):
    path = next((ROOT / 'docs/research').glob(f'{number:02d}-*.md'))
    rel = str(path.relative_to(ROOT))
    ids = re.findall(case_pattern, path.read_text(), re.M)
    for cid in ids:
        check(cid not in new_cases, 'Duplicate report case: ' + cid)
        check(cid in all_cases and cid not in old_cases, 'New case not integrated/new: ' + cid)
        new_cases[cid] = rel
    reports.append({'path': rel, 'new_cases': ids,
                    'pinned_source_citations': sum(ref[0] == rel for ref in source_refs)})
check(set(new_cases) == all_cases - old_cases, 'New-report/catalogue difference')

plan = (ROOT / 'docs/performance-plan.md').read_text()
rows = []
for line in plan.splitlines():
    if re.match(r'^\| PERF-\d{3} \|', line):
        rows.append([cell.strip() for cell in line.strip('|').split('|')])
tasks = json.loads((ROOT / 'docs/build-plan/performance-tasks.json').read_text())
check([row[0] for row in rows] == [f'PERF-{i:03d}' for i in range(1, 71)], 'Task sequence/duplicate')
check(tasks['task_count'] == len(rows) == 70, 'Task count')
for task, row in zip(tasks['tasks'], rows):
    check([task['id'], task['gates_and_dependencies'], task['experiment'], task['evidence_and_decision']] == row,
          'Task index mismatch: ' + task['id'])
    check(task['status'] == 'planned' and task['result'] is None, 'Unmeasured task has result')
check(len(tasks['tasks']) == len(rows), 'Task index length')

# Check the newly recorded Linux probes without rerunning them.
prior_tasks = json.loads((BASE / 'prior-task-ids.json').read_text())
check(prior_tasks == [f'PERF-{i:03d}' for i in range(1, 55)], 'Prior performance tasks mismatch')
check([t['id'] for t in tasks['tasks']][:len(prior_tasks)] == prior_tasks, 'Prior task IDs changed')
probe = json.loads((BASE / 'probe-results.json').read_text())
for kind in ['runner', 'source', 'compiler']:
    path = Path(probe[kind]['path'])
    if not path.is_absolute():
        path = ROOT / path
    check(digest(path) == probe[kind]['sha256'], 'Changed probe ' + kind)
check(probe['all_expected_observations'], 'Unexpected Linux observation')
check(probe['compile_invocations'] == len(probe['profiles']) == 2, 'Compile count mismatch')
check({p['name'] for p in probe['profiles']} == {'unoptimized', 'optimized'}, 'Profile coverage')
commands = [probe['compiler']['command']]
executions = 0
for row in probe['profiles']:
    commands.append(row['compile'])
    check(digest(ROOT / row['artifact']['path']) == row['artifact']['sha256'], 'Changed binary')
    check([r['case'] for r in row['executions']] == list(probe['expected']), 'Scenario coverage')
    for entry in row['executions']:
        command = entry['command']
        commands.append(command)
        executions += 1
        observed = json.loads((ROOT / command['stdout']).read_text())
        check(observed == entry['observed'] == probe['expected'][entry['case']], 'Wrong observed result')
        check(entry['expected_observation'], 'Unexpected case')
        check((ROOT / command['stderr']).stat().st_size == 0, 'Unexpected execution stderr')
for command in commands:
    check(not command['timed_out'] and command['exit_code'] == 0, 'Probe command failed/timed out')
    for stream in ['stdout', 'stderr']:
        check(digest(ROOT / command[stream]) == command[stream + '_sha256'], 'Changed raw output')
check(executions == probe['execution_invocations'] == 10, 'Execution count mismatch')
probe_summary = {'result_path': str((BASE / 'probe-results.json').relative_to(ROOT)),
                 'result_sha256': digest(BASE / 'probe-results.json'),
                 'scenario_families': len(probe['expected']), 'compiler_invocations': 2,
                 'executions': executions, 'expected_observations': probe['all_expected_observations'],
                 'scope': 'Sequential Linux ABI facts, not Bream concurrency, runtime qualification or performance.'}
retained = json.loads((BASE / 'prior-evidence-hashes.json').read_text())
for record in retained:
    check(digest(ROOT / record['path']) == record['sha256'], 'Changed historical evidence: ' + record['path'])

head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=OTP, text=True).strip()
status = subprocess.check_output(['git', 'status', '--porcelain'], cwd=OTP, text=True).strip()
check(head == PIN and not status, 'Pinned source changed')
result = {
    'review_date': '2026-10-07', 'status': 'failed' if errors else 'passed', 'errors': errors,
    'scope': 'Documentation, source ranges, stable IDs, planned tasks and recorded Linux syscall probes; not Bream runtime qualification.',
    'source_reference': {'release': 'OTP-29.1.1', 'commit': head, 'checkout_clean': not status},
    'counts': {'markdown_documents': len(docs), 'local_link_occurrences': local_links,
               'pinned_source_citations': len(source_refs), 'unique_cited_pinned_files': len(source_paths),
               'feature_groups': len(parsed), 'branch_cases': len(all_cases), 'new_branch_cases': len(new_cases),
               'new_reports': len(reports), 'planned_performance_tasks': len(rows)},
    'case_counts_by_document': by_document, 'reports': reports, 'prior_447_ids_preserved': not (old_cases - all_cases),
    'new_beam_execution': False, 'new_model_execution': False, 'performance_benchmarks_run': False,
    'linux_syscall_probes': probe_summary,
    'production_implementation': False, 'retained_historical_evidence': retained,
    'previous_rust_review_sha256': digest(ROOT / 'evidence/rust-ownership-review/review-validation.json'),
    'document_hashes': [{'path': str(p.relative_to(ROOT)), 'sha256': digest(p)} for p in docs +
                        [ROOT / 'docs/build-plan/features.json', ROOT / 'docs/build-plan/performance-tasks.json']],
    'pinned_source_hashes': [{'path': p, 'sha256': digest(OTP / p)} for p in sorted(source_paths)],
    'validator': {'path': str(Path(__file__).relative_to(ROOT)), 'sha256': digest(Path(__file__))},
}
(BASE / 'review-validation.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'status': result['status'], 'counts': result['counts'], 'errors': errors}, indent=2))
raise SystemExit(bool(errors))
