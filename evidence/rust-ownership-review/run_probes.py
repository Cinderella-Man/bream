"""Isolated Rust-language observations, not Bream admission or speed tests."""
from pathlib import Path
import hashlib
import json
import os
import signal
import subprocess
import sys

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[1]
RUSTC = Path('/home/bat/projects/rust-actor-runtime-lab/.tools/rustup/toolchains/1.98.1-x86_64-unknown-linux-gnu/bin/rustc')
ARTIFACTS = ROOT / '.tools/rust-ownership-review'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def invoke(command, output, timeout):
    process = subprocess.Popen([str(x) for x in command], cwd=BASE,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               text=True, start_new_session=True)
    timed_out = False
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
    output.parent.mkdir(parents=True, exist_ok=True)
    out, err = Path(str(output) + '.stdout'), Path(str(output) + '.stderr')
    out.write_text(stdout)
    err.write_text(stderr)
    return {'command': [str(x) for x in command], 'cwd': str(BASE),
            'exit_code': process.returncode, 'timed_out': timed_out,
            'stdout': str(out.relative_to(ROOT)), 'stderr': str(err.relative_to(ROOT)),
            'stdout_sha256': sha(out), 'stderr_sha256': sha(err)}, stdout, stderr


def main():
    probes = json.loads((BASE / 'probes.json').read_text())
    version = subprocess.check_output([RUSTC, '--version', '--verbose'], text=True)
    results = []
    for profile, level in [('unoptimized', '0'), ('optimized', '3')]:
        for probe in probes:
            binary = ARTIFACTS / profile / probe['name']
            binary.parent.mkdir(parents=True, exist_ok=True)
            source = BASE / probe['source']
            output = BASE / 'runs' / profile / probe['name']
            compile_result, _, stderr = invoke(
                [RUSTC, '--edition=2024', '-C', 'opt-level=' + level,
                 '--error-format=json', source, '-o', binary],
                Path(str(output) + '.compile'), 40)
            codes = []
            for line in stderr.splitlines():
                try:
                    diagnostic = json.loads(line)
                except ValueError:
                    continue
                if diagnostic.get('level') == 'error' and diagnostic.get('code'):
                    codes.append(diagnostic['code']['code'])
            expected = not compile_result['timed_out']
            expected &= ((compile_result['exit_code'] != 0 and probe['expected'] in codes)
                         if probe['kind'] == 'reject' else compile_result['exit_code'] == 0)
            row = {'profile': profile, 'name': probe['name'], 'kind': probe['kind'],
                   'source': str(source.relative_to(ROOT)), 'source_sha256': sha(source),
                   'compile': compile_result, 'diagnostic_codes': codes}
            if compile_result['exit_code'] == 0:
                row['artifact'] = {'path': str(binary.relative_to(ROOT)), 'sha256': sha(binary)}
            if expected and probe['kind'] == 'run':
                execution, stdout, _ = invoke([binary], Path(str(output) + '.execute'), 5)
                expected &= execution['exit_code'] == 0 and not execution['timed_out'] and stdout == probe['expected']
                row['execution'] = execution
                row['observed_stdout'] = stdout
            row['expected_observation'] = bool(expected)
            results.append(row)
    record = {
        'date': '2026-10-07', 'scope': 'Rust type/ownership/compiler observations only; no Bream implementation, preemption qualification or benchmark.',
        'rustc': {'path': str(RUSTC), 'version_verbose': version, 'sha256': sha(RUSTC)},
        'runner': {'path': str(Path(__file__).relative_to(ROOT)), 'sha256': sha(Path(__file__))},
        'manifest_sha256': sha(BASE / 'probes.json'), 'profiles': ['unoptimized', 'optimized'],
        'compile_invocations': len(results), 'execution_invocations': sum('execution' in r for r in results),
        'all_expected_observations': all(r['expected_observation'] for r in results),
        'results': results,
        'limits': ['Compiler rejection does not prove Bream admission soundness.',
                   'Accepted counterexamples are intentionally ordinary safe Rust, not admitted Bream programs.',
                   'The non-returning callback fixture was compiled but never executed.',
                   'The async fixture uses an explicit await to inspect Rust-generated storage; it is not Bream user syntax or an automatic preemption test.',
                   'No speed, allocator cost, arbitrary destruction bound or concurrent reclamation result was measured.']}
    (BASE / 'probe-results.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps({k: record[k] for k in ['compile_invocations', 'execution_invocations', 'all_expected_observations']}, indent=2))
    return 0 if record['all_expected_observations'] else 1


if __name__ == '__main__':
    sys.exit(main())
