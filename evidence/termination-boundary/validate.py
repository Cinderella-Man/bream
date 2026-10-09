"""Read-only verification of the passing cohort; no experiment execution."""
import hashlib
import json
from pathlib import Path


BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[1]
RESULT = BASE / 'runs/20261009-boundary-03/results.json'


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    record = json.loads(RESULT.read_text())
    checks = []

    def check(name, passed):
        checks.append({'check': name, 'passed': bool(passed)})

    def hashed(path, expected):
        path = Path(path)
        path = path if path.is_absolute() else ROOT / path
        check(str(path), path.is_file() and sha(path) == expected)

    check('passing cohort', record['all_expected_observations'] and 'error' not in record)
    check('command count', len(record['commands']) == 26)
    check('observation count', len(record['observations']) == 30)
    for row in record['source_inputs']:
        hashed(row['source'], row['sha256'])
        hashed(row['retained'], row['sha256'])
    outputs = {}
    for row in record['commands']:
        check(row['name'] + ': successful', row['succeeded'] and row['exit_code'] == 0 and not row['timed_out'])
        for stream in ['stdout', 'stderr']:
            hashed(row[stream], row[stream + '_sha256'])
        outputs[row['name']] = (ROOT / row['stdout']).read_text()
    seen = set()
    binaries = {}
    for row in record['observations']:
        key = (row['language'], row['profile'], row['case'])
        check(str(key) + ': unique and exact', key not in seen and row['matches'] and row['actual'] == row['expected'])
        seen.add(key)
        if row['language'] == 'rust':
            actual = json.loads(outputs[row['profile'] + '-' + row['case']])
            check(str(key) + ': retained output', actual == row['actual'])
            binaries[row['binary']] = row['binary_sha256']
        else:
            actual = json.loads(outputs['beam-' + row['profile']])
            matches = [x for x in actual['cases'] if x['case'] == row['case']]
            check(str(key) + ': retained output', matches == [row['actual']])
    for profile in ['unoptimized', 'optimized']:
        check(profile + ': nine cases', sum(k[:2] == ('rust', profile) for k in seen) == 9)
    for profile in ['jit', 'debug-emu']:
        check(profile + ': six cases', sum(k[:2] == ('erlang', profile) for k in seen) == 6)
    for path, expected in binaries.items():
        hashed(path, expected)
    hashed(record['rustc']['path'], record['rustc']['sha256'])
    otp = record['otp']
    for field in ['prior_build_provenance', 'retained_build_provenance']:
        hashed(otp[field], otp['provenance_sha256'])
    provenance = json.loads((ROOT / otp['retained_build_provenance']).read_text())
    check('OTP source provenance', otp['commit'] == otp['build_commit'] == provenance['source_commit']
          == 'ad05823719d77c8faee87348ea39513d4e2f99c5')
    for artifact in provenance['binary_hashes']:
        hashed(artifact['path'], artifact['sha256'])
    for artifact in otp['emulators']:
        hashed(artifact['path'], artifact['sha256'])
    hashed(ROOT / '.tools/termination-boundary/20261009-boundary-03/termination_reference.beam',
           otp['fixture_sha256'])
    report = {'scope': 'read-only hashes, retained observations and provenance; no execution or speed evidence',
              'result': str(RESULT.relative_to(ROOT)), 'result_sha256': sha(RESULT),
              'checks': checks, 'all_passed': all(row['passed'] for row in checks)}
    (BASE / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'checks': len(checks), 'all_passed': report['all_passed']}))
    return 0 if report['all_passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
