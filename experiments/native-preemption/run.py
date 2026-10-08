#!/usr/bin/env python3
"""Run an already-built native mechanism probe under an external deadline."""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import resource
import subprocess
import time

HERE = Path(__file__).resolve().parent
EVIDENCE = HERE.parents[1] / 'evidence' / 'native-preemption'

def disable_core():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--label', required=True)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--expect-preemption', choices=['yes', 'no'], required=True)
    args = parser.parse_args()
    binary = Path(args.binary).resolve()
    output = EVIDENCE / args.label
    output.mkdir(parents=True, exist_ok=False)
    started = time.monotonic()
    record = dict(started_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  argv=[str(binary)], external_deadline_seconds=10,
                  expect_preemption=args.expect_preemption,
                  input_hashes={str(p):hashlib.sha256(p.read_bytes()).hexdigest()
                                for p in [binary, HERE/'Cargo.toml', HERE/'Cargo.lock', HERE/'src/main.rs']})
    try:
        run = subprocess.run([str(binary)], cwd=HERE, capture_output=True,
                             timeout=10, preexec_fn=disable_core)
        (output/'stdout.jsonl').write_bytes(run.stdout)
        (output/'stderr.log').write_bytes(run.stderr)
        record['exit_code'] = run.returncode
        parsed = json.loads(run.stdout.decode().strip())
        record['observation'] = parsed
        if args.expect_preemption == 'yes':
            record['as_expected'] = run.returncode == 0 and parsed['passed']
        else:
            record['as_expected'] = (run.returncode == 1 and not parsed['passed']
                                     and parsed['complete'] and parsed['same_worker']
                                     and not parsed['both_observed_overlap'])
    except subprocess.TimeoutExpired as error:
        (output/'stdout.jsonl').write_bytes(error.stdout or b'')
        (output/'stderr.log').write_bytes(error.stderr or b'')
        record['timeout'] = True
        record['as_expected'] = False
    except Exception as error:
        record['error'] = repr(error)
        record['as_expected'] = False
    record['elapsed_seconds'] = time.monotonic()-started
    (output/'result.json').write_text(json.dumps(record,indent=2)+'\n')
    print(json.dumps(record))
    return 0 if record['as_expected'] else 1

if __name__ == '__main__':
    raise SystemExit(main())
