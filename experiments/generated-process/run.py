#!/usr/bin/env python3
"""Build and exercise the closed research subset, retaining raw bounded evidence."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import resource
import signal
import subprocess
import time

HERE = Path(__file__).resolve().parent
LAB = Path('/home/bat/projects/rust-actor-runtime-lab')
CARGO = LAB / '.tools/cargo/bin/cargo'
RUSTC = LAB / '.tools/cargo/bin/rustc'
ENV = dict(os.environ, CARGO_HOME=str(HERE/'.cargo-home'),
           RUSTUP_HOME=str(LAB/'.tools/rustup'),
           PATH=str(CARGO.parent)+os.pathsep+os.environ.get('PATH',''))

def no_core():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))

def bounded(argv, output, label, seconds):
    with (output/f'{label}.stdout').open('wb') as stdout, (output/f'{label}.stderr').open('wb') as stderr:
        process = subprocess.Popen(argv, cwd=HERE, env=ENV, stdout=stdout, stderr=stderr,
                                   start_new_session=True, preexec_fn=no_core)
        timed_out = False
        try:
            code = process.wait(timeout=seconds)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGKILL)
            code = process.wait()
    return dict(argv=[str(a) for a in argv], exit_code=code, timed_out=timed_out,
                external_deadline_seconds=seconds,
                stdout=str(output/f'{label}.stdout'), stderr=str(output/f'{label}.stderr'))

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--label', required=True)
    args = parser.parse_args()
    output = HERE.parents[1]/'evidence/generated-process'/args.label
    output.mkdir(parents=True, exist_ok=False)
    started = time.monotonic()
    record = dict(started_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  platform=platform.platform(), passed=False, steps=[])
    try:
        record['rustc'] = subprocess.check_output([RUSTC, '-Vv'],env=ENV,text=True)
        build = bounded([CARGO, 'build', '--release', '--locked', '--bin', 'bream-probe-runtime', '-j3'], output, 'build', 120)
        record['steps'].append(build)
        assert build['exit_code'] == 0 and not build['timed_out'], 'build failed'
        binary = HERE/'target/release/bream-probe-runtime'
        execution = bounded([binary], output, 'execution', 10)
        record['steps'].append(execution)
        assert execution['exit_code'] == 0 and not execution['timed_out'], 'execution failed'
        observation = json.loads(Path(execution['stdout']).read_text())
        record['observation'] = observation
        assert observation['passed'] and observation['killed_after_pending']
        assert observation['live_bytes_after_drop'] == 0 and observation['buffer_drops_during_poll'] == 0
        assert observation['survivor_final_progress'] > observation['survivor_progress_at_kill'] > 0
        rejects = []
        for fixture in sorted((HERE/'runtime/examples').glob('reject_*.rs')):
            check = bounded([CARGO, 'check', '--release', '--locked', '-p', 'bream-probe-runtime', '--example', fixture.stem, '-j3'], output, fixture.stem, 60)
            record['steps'].append(check)
            diagnostics = Path(check['stderr']).read_text()
            expected = check['exit_code'] != 0 and not check['timed_out'] and 'bream research subset:' in diagnostics
            rejects.append(dict(fixture=str(fixture), rejected_by_subset=expected))
            assert expected, f'{fixture.name} did not fail through subset checker'
        record['compile_fail'] = rejects
        inputs = [HERE/'Cargo.toml', HERE/'Cargo.lock', HERE/'run.py', binary]
        for directory in [HERE/'macros',HERE/'runtime']:
            inputs.extend(sorted(directory.rglob('*.rs')))
            inputs.append(directory/'Cargo.toml')
        record['inputs'] = [{'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in inputs]
        record['passed'] = True
    except Exception as error:
        record['error'] = repr(error)
    record['elapsed_seconds'] = time.monotonic()-started
    (output/'result.json').write_text(json.dumps(record,indent=2)+'\n')
    print(json.dumps(record))
    return 0 if record['passed'] else 1

if __name__ == '__main__':
    raise SystemExit(main())
