#!/usr/bin/env python3
"""Retain correctness evidence only. This machine must never run speed tests."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
BASE = Path(__file__).resolve().parent
PROJECT = ROOT / 'experiments/busy-session'
TOOLCHAIN = Path('/home/bat/projects/rust-actor-runtime-lab/.tools/rustup/toolchains/1.98.1-x86_64-unknown-linux-gnu/bin')
assert len(sys.argv) == 2, 'Supply a unique correctness run name'
RUN = BASE / 'runs' / sys.argv[1]
RUN.mkdir(parents=True, exist_ok=False)
TARGET = ROOT / '.tools/busy-session' / RUN.name
OVERRIDES = {'CARGO_HOME': str(ROOT / '.tools/cargo-cache'), 'CARGO_TARGET_DIR': str(TARGET),
             'RUSTC': str(TOOLCHAIN / 'rustc'), 'CARGO_BUILD_JOBS': '2', 'RUSTFLAGS': '', 'RUSTDOCFLAGS': '',
             'RUSTC_WRAPPER': '', 'RUSTC_WORKSPACE_WRAPPER': '', 'RUST_BACKTRACE': '0'}
ENV = os.environ.copy()
ENV.pop('CARGO_ENCODED_RUSTFLAGS', None)
ENV.update(OVERRIDES)
commands, observations, artifacts, errors = [], [], [], []

def record(path):
    path = Path(path)
    return {'path': str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path),
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}

def retained_input(path):
    copy = RUN / 'inputs' / path.relative_to(ROOT)
    copy.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(path, copy)
    return {'original_path': str(path.relative_to(ROOT)), **record(copy)}

def invoke(name, argv, timeout=60):
    child = subprocess.Popen([str(a) for a in argv], cwd=ROOT, env=ENV, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    expired = False
    try:
        out, err = child.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        expired = True
        os.killpg(child.pid, signal.SIGKILL)
        out, err = child.communicate()
    stdout, stderr = RUN / (name+'.stdout'), RUN / (name+'.stderr')
    stdout.write_bytes(out); stderr.write_bytes(err)
    command = {'name': name, 'argv': [str(a) for a in argv], 'exit_code': child.returncode,
               'timed_out': expired, 'watchdog_seconds': timeout, 'stdout': record(stdout), 'stderr': record(stderr)}
    commands.append(command)
    return command, out.decode(), err.decode()

protocol = {
    'date': '2026-10-08', 'milestone': 'Room and healthy peer progress beside a continuously computing member, stop without receive, shared-reference cleanup and active shutdown',
    'correctness_only': True, 'speed_tests': False, 'performance_comparisons': False,
    'machine_constraint': 'User actively uses this machine for other work. NEVER run speed tests or performance comparisons here; prior local timings are unsuitable performance indicators.',
    'allowances': [1, 4, 32], 'build_jobs': 2,
    'controls': 'Each compiler profile: 95 deterministic protocol/ownership tests; three inherited mailbox and three timeout controls, three monitor controls, three link controls, three spawn controls, three room controls, three immutable-sharing controls and three busy-member controls; 148 compile-rejected consumers and eight positive shadowing routes.',
    'busy_oracles': 'Room and healthy member complete3requests while a monitored broadcast recipient computes indefinitely; logical busy progress strictly increases with each protocol checkpoint without durations or rates. Direct kill and linked killed stop the busy process with only its initial receive; room matches exact Down ref/PID/reason;2post-stop replies still read valid shared bytes; application state remains frozen. Active shutdown settles all3actors.19shared plus3Words queued events retire in3batches;23createdreferences stay constant through helper calls, then20release on busy death,22before host drop and23after final physical release. Original process inputs and forwarded request receipts settle once. No numeric peer/stop latency guarantee.',
    'shared_oracles': 'Zero/one/two-recipient checked broadcast and receiver failure; checksum read after publisher kill and peer kill; one physical backing independent of references; capacity admission; clone allocation identity; rejected send consumes its reference; host final owner; two-thread final release; partial-fanout cancellation reference accounting; cancellation of suspended cloned send operand before publication; typed mailbox selection and injected timeout;19queued releases in3batches;16shared send/kill races per invocation. Partial-fanout cancellation does not inspect child checksum; the separate known-handle death scenarios do.',
    'room_oracles': 'Two fixed session memberships; idempotent duplicate join and repeated leave; unknown member and unavailable target replies; single-target owned routing; exact initial child reference and PID matching; final session report before Down; independent CPU hog advances while room and surviving session complete transcript; timeout preserves one unrelated publish; forced room death and host shutdown settle a held request plus19queued requests and all input/message receipts; invalid session notifications do not terminate the session.',
    'spawn_oracles': 'Full origin identity under registry serialization; initial link/monitor before dispatch; actual first poll sees topology; fresh child trap flag; pending kill may publish before actual stop; normal parent stop preserves default child; abnormal linked stop propagates while monitor-only does not; returned references match actual child reason; monitor exhaustion and dispatch failure roll back topology and input; moved Words keeps original allocation receipt; closed checked child graph/factory/result admission.',
    'link_oracles': 'Idempotent/self links; synchronous absent-link error and explicit typed recovery; current trap state at conversion; unlink suppresses internal but not converted Exit; same-full-Pid relink may reactivate older exit; full process incarnation prevents slot aliasing; direct forced kill remains untrappable while propagated killed is trappable; linked Exit converts before Down; max8 combined link+monitor items per cleanup; actual peer poll between cleanup batches.',
    'monitor_oracles': 'Full incarnation identity; self-monitor installs nothing; stop before Down conversion; cancellation suppresses pending internal Down but leaves converted Down; explicit flush preserves unrelated events; weak registrations and bounded observer cleanup; cancelled pending entries removed and wake commands coalesced.',
    'logical_oracles': 'Zero scans before timeout; finite wait arms only after initial scan; unmatched arrivals do not rearm; committed expiry selects timeout before resumed scan; cancelled/detached callback cannot affect another receive or incarnation; exact ownership; max8 expiry callbacks per turn with application work during a19-timer burst; braced/formatted nested matches have identical logical checkpoint charges.',
    'races': 'Each spawn invocation:16 spawn/parent-kill and16 spawn/shutdown races. Each link invocation:16 link/target-kill and16 unlink/target-kill races. Each monitor invocation:16 creation/target-kill and16 cancellation/target-kill races. Each timeout invocation:16 send/timeout races and16 kill/timeout races. Each inherited mailbox invocation:16 send/kill races and16 concurrent-sender messages. Allowed outcome and ownership assertions only.',
    'timing_policy': 'No sampled timings, rates, warmups, affinity changes or performance summaries. Watchdog timeouts detect hangs only; incidental cargo test/build duration text is not analyzed as evidence.',
    'limits': ['one application worker', 'concrete Words and immutable-byte application-envelope schemas', 'fixed scalar tag or any selector',
               'trusted host allocator and panic behavior', 'logical clock explicitly advanced by host; no real-time timer driver', 'finite delay range0..u32::MAX; no general exception catch; try_link maps synchronous NoProcess', 'local genuine Down and linked Exit only; no explicit exit/1 or exit/2, synthetic notifications or distribution', 'no general worker-failure recovery', 'no full G1/G2/G3/G4 qualification', 'room and broadcast fixtures have2fixed members; one busy member is covered, but no dynamic membership or general sender identity; immutable bytes enter only from trusted host; no slices, mutable sharing or arbitrary shared Rust types', 'child function is statically selected in checked module; acyclic helper/spawn graph; no asynchronous/distributed spawn or arbitrary spawn options'],
}
(RUN / 'protocol.json').write_text(json.dumps(protocol, indent=2)+'\n')
inputs = [retained_input(p) for p in sorted(PROJECT.rglob('*')) if p.is_file() and p.suffix in ['.rs','.toml','.lock']]
inputs.append(retained_input(Path(__file__).resolve()))
try:
    c, out, err = invoke('rustc-version', [TOOLCHAIN/'rustc', '-vV'])
    assert c['exit_code'] == 0 and 'rustc 1.98.1' in out
    common = ['--offline', '--locked', '--manifest-path', PROJECT/'Cargo.toml']
    build = [TOOLCHAIN/'cargo', 'build', *common, '-p', 'bream-busy-consumer']
    negatives = sorted((PROJECT/'consumer/examples').glob('reject_*.rs'))
    diagnostic = {
        'reject_future':'mismatched types', 'reject_closure_spawn':'mismatched types',
        'reject_safe_constructor':'requires unsafe function or block', 'reject_program_fields':'is private',
        'reject_drop_input':'mismatched types', 'reject_struct_duplicate_owner':'E0382', 'reject_struct_use_after_move':'E0382',
        'reject_struct_missing_arm':'E0004', 'reject_struct_private_type':'E0603', 'reject_struct_private_helper':'E0603',
        'reject_id_fields':'E0451', 'reject_id_mutation':'E0616', 'reject_send_twice':'E0382', 'reject_message_twice':'E0382',
        'reject_envelope_clone':'E0599', 'reject_send_custom_payload':'E0308',
        'reject_timeout_missing_arm':'E0004', 'reject_timeout_duplicate_owner':'E0382',
        'reject_monitor_missing_arm':'E0004', 'reject_monitor_duplicate_owner':'E0382',
        'reject_link_missing_arm':'E0004', 'reject_link_try_missing_arm':'E0004',
        'reject_spawn_duplicate_owner':'E0382',
        'reject_shared_missing_arm':'E0004', 'reject_shared_event_missing_arm':'E0004',
        'reject_shared_duplicate_owner':'E0382', 'reject_shared_duplicate_message':'E0382',
    }
    new_grammar = {'reject_receive_predicate','reject_send_numeric_pid','reject_pid_arithmetic','reject_message_constructor','reject_message_builtin_shadow'}
    for profile in ['debug','release']:
        flags = ['--release'] if profile == 'release' else []
        c, out, err = invoke('unit-'+profile, [TOOLCHAIN/'cargo','test',*common,'-p','bream-busy-runtime','--lib',*flags])
        assert c['exit_code'] == 0 and not c['timed_out'] and '95 passed; 0 failed' in out, (out, err)
        observations.append({'kind':'unit_tests','command':c['name'],'profile':profile,'tests':95,'passed':True})
        c, out, err = invoke('build-'+profile, build+flags)
        assert c['exit_code'] == 0 and not c['timed_out'], err
        binary = TARGET/profile/'bream-busy-consumer'; artifacts.append(record(binary))
        for allowance in protocol['allowances']:
            c, out, err = invoke(f'controls-{profile}-{allowance}', [binary,'controls',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'] and not err, (out, err)
            row = json.loads(out)
            assert row['passed'] and row['speed_tests'] is False and row['scan_candidates'] == 129
            observations.append({'command':c['name'],'profile':profile,**row})
        for allowance in protocol['allowances']:
            c, out, err = invoke(f'timeouts-{profile}-{allowance}', [binary,'timeouts',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'], (out, err)
            assert 'receive timeout exceeds u32 range' in err and 'receive timer failed: DeadlineOverflow' in err, err
            row = json.loads(out)
            assert row['passed'] and row['speed_tests'] is False and row['burst_service_batches'] == 3
            observations.append({'command':c['name'],'profile':profile,**row})
        for allowance in protocol['allowances']:
            c, out, err = invoke(f'monitors-{profile}-{allowance}', [binary,'monitors',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'], (out, err)
            row = json.loads(out)
            assert row['passed'] and row['speed_tests'] is False
            observations.append({'command':c['name'],'profile':profile,**row})
        for allowance in protocol['allowances']:
            c, out, err = invoke(f'links-{profile}-{allowance}', [binary,'links',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'], (out, err)
            assert 'link failed: NoProcess' in err, err
            row = json.loads(out)
            assert row['passed'] and row['speed_tests'] is False
            observations.append({'command':c['name'],'profile':profile,**row})
        for allowance in protocol['allowances']:
            c, out, err = invoke(f'spawns-{profile}-{allowance}', [binary,'spawns',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'], (out, err)
            assert 'child spawn failed: Capacity' in err and 'input already taken' in err and 'index out of bounds' in err, err
            row = json.loads(out)
            assert row['passed'] and row['speed_tests'] is False
            assert row['spawn_parent_kill_races'] == row['spawn_shutdown_races'] == 16
            assert row['parent_race_published'] + row['parent_race_rejected'] == 16
            assert row['shutdown_race_published'] + row['shutdown_race_rejected'] == 16
            observations.append({'command':c['name'],'profile':profile,**row})
        for allowance in protocol['allowances']:
            c, out, err = invoke(f'room-{profile}-{allowance}', [binary,'room',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'], (out, err)
            assert 'input already taken' in err, err
            row = json.loads(out)
            assert row['passed'] and row['speed_tests'] is False
            assert row['observed_records'] == len(row['expected_transcript']) == 15
            digests = {}
            expected_digest = 0
            for count, event in enumerate(row['expected_transcript'],1):
                assert len(event) == 11 and event[8] == 0 and event[9] == count
                for value in event:
                    expected_digest = (expected_digest*131+value) % (1<<64)
                digests[count] = (expected_digest,event[9])
            assert row['observed_digest'] == expected_digest
            assert [check[0] for check in row['observed_checkpoints']] == [1,2,3,4,5,6,7,8,10,11,12,13,14,15]
            for count, digest, sequence in row['observed_checkpoints']:
                assert (digest,sequence) == digests[count]
            observations.append({'command':c['name'],'profile':profile,**row})
        for allowance in protocol['allowances']:
            c, out, err = invoke(f'shared-{profile}-{allowance}', [binary,'shared',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'], (out, err)
            assert 'index out of bounds' in err, err
            row = json.loads(out)
            checksum = 0
            for byte in [2,3,5,7]: checksum = checksum*31+byte
            assert row['passed'] and row['speed_tests'] is False and row['unpublished_operand_cancelled']
            assert row['broadcast_observations'] == [[0,0,0,0],[1,0,checksum+1,1],[2,0,checksum*2+3,2],[2,1,checksum+2,1]]
            assert row['after_publisher_checksum'] == row['after_receiver_checksum'] == checksum
            assert (row['cleanup_messages'],row['cleanup_batches'],row['shared_send_kill_races']) == (19,3,16)
            observations.append({'command':c['name'],'profile':profile,**row})
        for allowance in protocol['allowances']:
            c, out, err = invoke(f'busy-{profile}-{allowance}', [binary,'busy',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'] and not err, (out, err)
            row = json.loads(out)
            assert row['passed'] and row['speed_tests'] is False
            assert len(row['progress_observations']) == 3
            for scenario in row['progress_observations']:
                assert len(scenario) == 3
                for seq, (actual, digest, before, after) in enumerate(scenario,1):
                    assert (actual,digest) == (seq,626270+seq) and after > before >= 2
            assert row['post_stop_observations'] == [[0,4,626274],[0,5,626275],[1,4,626274],[1,5,626275]]
            assert row['stop_observations'] == [[0,2,2,1,22,3,62627],[1,3,2,1,22,3,62627]]
            assert row['ownership_observations'] == [[[4,0,0,3,23,20],[4,0,0,1,23,22],[0,4,1,0,23,23]]]*2
            assert row['shutdown_ownership'] == [[4,0,0,1,23,22],[0,4,1,0,23,23]]
            assert row['shutdown_cleanup'] == [22,3] and row['shutdown_room_result'] == 626273 and row['shutdown_busy_result'] == 62627
            observations.append({'command':c['name'],'profile':profile,**row})
        for example in ['shadow_paths','shadow_structured','shadow_mailbox','shadow_timeout','shadow_monitor','shadow_link','shadow_spawn','shadow_shared']:
            c, out, err = invoke(example+'-build-'+profile, build+flags+['--example',example])
            assert c['exit_code'] == 0 and not c['timed_out'], err
            binary = TARGET/profile/'examples'/example; artifacts.append(record(binary))
            c, out, err = invoke(example+'-execute-'+profile, [binary])
            assert c['exit_code'] == 0 and out == example.upper()+'_PASSED\n' and not err, (out, err)
        for fixture in negatives:
            c, out, err = invoke('reject-'+profile+'-'+fixture.stem, build+flags+['--example',fixture.stem])
            default = 'bream structured subset:' if fixture.stem.startswith(('reject_struct_', 'reject_timeout_', 'reject_monitor_', 'reject_link_', 'reject_spawn_', 'reject_shared_')) or fixture.stem in new_grammar else 'bream milestone subset:'
            required = diagnostic.get(fixture.stem, default)
            assert c['exit_code'] != 0 and not c['timed_out'] and required in err, (fixture.name, err)
            observations.append({'kind':'rejection','command':c['name'],'profile':profile,'fixture':fixture.stem,'diagnostic':required,'passed':True})
except Exception as error:
    errors.append(repr(error))
result = {
    'status':'failed' if errors else 'passed', 'errors':errors, 'protocol':record(RUN/'protocol.json'),
    'inputs':inputs,'tools':[record(TOOLCHAIN/t) for t in ['rustc','cargo']], 'artifacts':artifacts,
    'commands':commands,'observations':observations,'environment_overrides':OVERRIDES,
    'correctness_only':True, 'speed_tests':False,'performance_comparisons':False,'new_beam_execution':False,'production_runtime':False,
}
(RUN/'results.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'status':result['status'],'commands':len(commands),'observations':len(observations),'inputs':len(inputs),'speed_tests':False,'errors':errors},indent=2))
raise SystemExit(bool(errors))
