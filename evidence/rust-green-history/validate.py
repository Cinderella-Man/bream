#!/usr/bin/env python3
"""Read-only artifact audit; no compiler, experiment, network, or timing work."""
import ast
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RUN = ROOT / 'evidence/rust-green-history/runs/20261009-green-02'
checks = 0


def check(condition, message):
    global checks
    assert condition, message
    checks += 1


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    record = json.loads((RUN / 'results.json').read_text())
    check(record['all_expected_observations'], 'passing cohort')
    check(record['archive_checksum_verified'], 'archive verification')
    check(len(record['commands']) == 26, '26 commands')
    check(len(record['observations']) == 20, '20 observations')
    for row in record['source_inputs']:
        check(sha(ROOT / row['retained']) == row['sha256'], row['retained'])
        check(sha(ROOT / row['source']) == row['sha256'], row['source'])
    p = record['provenance']
    check(sha(ROOT / p['retained']) == p['sha256'], 'retained provenance')
    check(sha(ROOT / 'evidence/rust-green-history/provenance.json') == p['sha256'], 'current provenance')
    provenance = json.loads((ROOT / p['retained']).read_text())
    archive = provenance['official_compiler']
    check(sha(ROOT / archive['archive_path']) == archive['archive_sha256'], 'official archive')
    for manifest in record['historical_source_manifests']:
        check(sha(ROOT / manifest['retained']) == manifest['sha256'], 'retained source manifest')
        check(sha(ROOT / manifest['manifest']) == manifest['sha256'], 'current source manifest')
        content = json.loads((ROOT / manifest['retained']).read_text())
        check(len(content['files']) == manifest['files_checked'], 'source count')
        for item in content['files']:
            check(sha(ROOT / manifest['checkout'] / item['path']) == item['sha256'], item['path'])
    for artifact in record['historical_artifacts']:
        check(sha(ROOT / artifact['path']) == artifact['sha256'], artifact['path'])
    for tool in record['tools']:
        check(sha(tool['path']) == tool['sha256'], tool['name'])
    commands = {row['name']: row for row in record['commands']}
    check(len(commands) == 26, 'unique commands')
    for row in record['commands']:
        check(row['matched_expectation'] and not row['timed_out'], row['name'])
        check(sha(ROOT / row['stdout']) == row['stdout_sha256'], row['stdout'])
        check(sha(ROOT / row['stderr']) == row['stderr_sha256'], row['stderr'])
        code = row['expected_rejection_code']
        check(row['exit_code'] == (1 if code else 0), 'expected exit code')
        if code:
            check(f'error[{code}]' in (ROOT / row['stderr']).read_text(), code)
    # Read frozen expected dictionaries without executing even the runner module.
    constants = {}
    for node in ast.parse((RUN / 'inputs/run.py').read_text()).body:
        if isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name):
            if node.targets[0].id in {'HISTORICAL', 'MODERN_CASES', 'REJECTIONS'}:
                constants[node.targets[0].id] = ast.literal_eval(node.value)
    expected_keys = set()
    for profile in ['unoptimized', 'optimized']:
        for baseline, const in [('historical', 'HISTORICAL'), ('modern', 'MODERN_CASES'), ('compile-rejection', 'REJECTIONS')]:
            expected_keys.update((baseline, profile, case) for case in constants[const])
    actual_keys = {(r['baseline'], r['profile'], r['case']) for r in record['observations']}
    check(actual_keys == expected_keys and len(actual_keys) == 20, 'complete observation matrix')
    for row in record['observations']:
        check(row['matches'], row['case'])
        if row['baseline'] == 'compile-rejection':
            check(row['expected_code'] == constants['REJECTIONS'][row['case']], 'rejection code')
            name = row['profile'] + '-' + Path(row['case']).stem
            check(commands[name]['expected_rejection_code'] == row['expected_code'], name)
        else:
            const = 'HISTORICAL' if row['baseline'] == 'historical' else 'MODERN_CASES'
            expected = {'case': row['case'], **constants[const][row['case']]}
            check(row['actual'] == row['expected'] == expected, row['case'])
            name = row['baseline'] + '-' + row['profile'] + '-' + row['case']
            check(json.loads((ROOT / commands[name]['stdout']).read_text()) == expected, name)
            check(sha(ROOT / row['binary']) == row['binary_sha256'], row['binary'])
    result = {'schema': 1, 'run': str(RUN.relative_to(ROOT)), 'passed': True,
              'checks': checks, 'commands': 26, 'observations': 20,
              'scope': 'source, tool, binary, output and exact semantic-oracle audit; no execution or performance data'}
    (ROOT / 'evidence/rust-green-history/validation.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
