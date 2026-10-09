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
import oracles

ROOT = Path(__file__).resolve().parents[2]
BASE = Path(__file__).resolve().parent
PROJECT = ROOT / 'experiments/timed-read'
TOOLCHAIN = Path('/home/bat/projects/rust-actor-runtime-lab/.tools/rustup/toolchains/1.98.1-x86_64-unknown-linux-gnu/bin')
assert len(sys.argv) == 2, 'Supply a unique correctness run name'
RUN = BASE / 'runs' / sys.argv[1]
RUN.mkdir(parents=True, exist_ok=False)
TARGET = ROOT / '.tools/timed-read' / RUN.name
OVERRIDES = {'CARGO_HOME': str(ROOT / '.tools/cargo-cache'), 'CARGO_TARGET_DIR': str(TARGET),
             'RUSTC': str(TOOLCHAIN / 'rustc'), 'CARGO_BUILD_JOBS': '2', 'RUSTFLAGS': '', 'RUSTDOCFLAGS': '',
             'RUSTC_WRAPPER': '', 'RUSTC_WORKSPACE_WRAPPER': '', 'RUST_BACKTRACE': '0', 'RUST_TEST_THREADS': '2', 'ERL_FLAGS': '+S 1:1 +SDcpu 1 +SDio 1', 'ERL_CRASH_DUMP': str(TARGET/'erl_crash.dump')}
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
    'date': '2026-10-09', 'milestone': 'Persistent local read timeouts with injected-clock arbitration and settlement before reuse',
    'correctness_only': True, 'speed_tests': False, 'performance_comparisons': False,
    'machine_constraint': 'User actively uses this machine for other work. NEVER run speed tests or performance comparisons here; prior local timings are unsuitable performance indicators.',
    'allowances': [1, 4, 32], 'build_jobs': 2,
    'controls': 'Each compiler profile: 153 bounded protocol/ownership tests; three inherited mailbox and three timeout controls, three monitor controls, three link controls, three spawn controls, three room controls, three immutable-sharing controls three busy-member controls and three replacement controls; 189 compile-rejected consumers and twelve positive shadowing routes.',
    'timed_read_oracles': 'One fixed deadline per finite request; zero one nonblocking attempt with no timer; read_next remains infinite. Pending expiry commits Timeout, running expiry waits for settlement, claimed Data/EOF/error wins and retry/skip becomes Timeout. Stop overrides both. Typed timer destination separates mailbox and stream generations. Public data/timeout/reuse sequence holds earlier bytes, seven stop boundaries and open-peer shutdown;16 data/timeout and16 kill/timeout races per configuration. Unit controls include detached stale callbacks, same-slot replacement, overflow, held claims, synthetic Interrupted/error and bounded expiry service. Six exact OTP socket reference rows distinguish positive-length partial timeout from Bream read-some success.',
    'stream_oracles': 'Max16 installed endpoints, fixed1..4096byte chunk perhostinstallation; one request at a time with nonwrapping identity; positive reads retain endpoint, EOF/error close andcacheterminal; old cancellation/completion/readiness cannot settle a new Running request. Publictwo-read plusEOF cases preserve firstbuffer across secondread andterminalclose;6stopboundaries+19queuedWords;capacityretainedwhenidle;stalePID;shutdown idle/pending/secondread;16secondwrite/kill races. Unitcontrolledpollsnapshot survives actorstop before realpoll andnoPOLLNVAL; bufferedbytes afterpeerclose; syntheticerror/duplicate;cancelRunning keepsBusyuntilsettled;requestexhaustionfailsclosed. No time measurements.',
    'read_oracles': 'One host-installed read per actor; max16 pending/ready operations, max4096 bytes each; owned socket and buffer until synchronous read returns; typed short read/EOF/unavailable; ready results hold admission until consumed or discarded; pending/ready/actor-held cleanup, kill/read races, stale identity and shutdown with open peers. Internal phase controls cover claimed-read storage surviving actor stop and duplicate completion; injected failure is not an observed native error. Wake failure closes the driver endpoint and settles operations; no unbounded retry under actor lifecycle locks. First completed read closes its endpoint.',
    'replacement_oracles': 'Capacity3 forces replacement into the sole slot vacated by the old member. Checked code requires fresh full PID and monitor reference. A real old reply and Down are queued before new incarnation creation; consumption is deferred with selective receive. One stale trusted-epoch request is also preserved and then rejected. Old-PID Words/shared sends discard ownership without reaching replacement; plain old-reference demonitor leaves queuedDown and currentmonitor intact. Normal/failure old exits both preserve replacement and survivor checksums. Explicit room kill and active shutdown settle3old events plus19shared refs in3cleanupbatches. Shared receipts separately count6references for conversations and25for teardown. Epoch is not sender authentication; no claim of delayed Down publication or global sender order.',
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
               'trusted host allocator and panic behavior', 'logical clock explicitly advanced by host; no real-time timer driver', 'finite delay range0..u32::MAX; no general exception catch; try_link maps synchronous NoProcess', 'local genuine Down/Exit; uncaught self_exit and eager local exit_to receipts for five reasons; no application catch, queued direct-signal backlog, synthetic notifications or distribution', 'optional sole read driver, one host-owned one-shot endpoint per actor, max16 pending/ready operations and4096 bytes each; delivered buffers follow actor ownership; host aliases must not read/change nonblocking mode; no repeated reads, kernel-submitted asynchronous read or generic future admission', 'optional separate persistent-read profile: one actor owner perendpoint, max16 endpointrecords includingidle/ready, one active request perendpoint, fixedhostchunksize1..4096, no endpointtransfer/halfclose/networkprotocol/async kernelrequest; EOF/error closeendpointandcacheterminal; no whole-runtime memory bound', 'timed reads use injected logical clock, u32 delay range, one driver attempt at zero, claimed terminal results may win serviced expiry; Timeout waits for claimed request settlement; no hard deadline or realtime clock; zero Interrupted maps Timeout; Clock expired counts intent while stream timeouts counts committed results', 'no general worker-failure recovery', 'no full G1/G2/G3/G4 qualification', 'room and broadcast fixtures have2fixed members; one busy member and one bounded membership replacement are covered, but no general dynamic collection or authenticated application sender identity; immutable bytes enter only from trusted host; no slices, mutable sharing or arbitrary shared Rust types', 'child function is statically selected in checked module; acyclic helper/spawn graph; no asynchronous/distributed spawn or arbitrary spawn options'],
}
(RUN / 'protocol.json').write_text(json.dumps(protocol, indent=2)+'\n')
inputs = [retained_input(p) for p in sorted(PROJECT.rglob('*')) if p.is_file() and p.suffix in ['.rs','.toml','.lock']]
inputs += [retained_input(p) for p in [Path(__file__).resolve(), BASE/'oracles.py', PROJECT/'exit_reference.erl', PROJECT/'timed_read_reference.erl', ROOT/'evidence/process-protocol-review/source-build.json']]
protocol['exit_oracles'] = 'Thirty exact normal/killed/failure/noproc/kill by self-exit/self-target/peer-target and trap cases; checked helper nonreturn; held Words/shared refs plus19 queued shares;3cleanup batches; two computing-target stops; stale-PID checked send; exact source/reason and topology in unit tests;16 kill/direct-failure races per profile; pinned OTP jit+debug39-row reference, no application catch admitted.'
(RUN/'protocol.json').write_text(json.dumps(protocol,indent=2)+'\n')
try:
    c, out, err = invoke('rustc-version', [TOOLCHAIN/'rustc', '-vV'])
    assert c['exit_code'] == 0 and 'rustc 1.98.1' in out
    otp = ROOT/'upstream/otp-reference-build'
    expected_commit = 'ad05823719d77c8faee87348ea39513d4e2f99c5'
    for name, directory in [('otp-source-commit', ROOT/'upstream/otp'), ('otp-build-commit', otp)]:
        c, out, err = invoke(name, ['git','-C',directory,'rev-parse','HEAD'])
        assert c['exit_code'] == 0 and out.strip() == expected_commit
    provenance = json.loads((ROOT/'evidence/process-protocol-review/source-build.json').read_text())
    assert provenance['source_commit'] == expected_commit
    for artifact in provenance['binary_hashes']:
        assert record(ROOT/artifact['path'])['sha256'] == artifact['sha256']
    beam = TARGET/'beam'; beam.mkdir(parents=True, exist_ok=True)
    c, out, err = invoke('beam-compile', [otp/'bin/erlc','-Werror','+debug_info','-o',beam,PROJECT/'exit_reference.erl'])
    assert c['exit_code'] == 0 and not c['timed_out'], (out,err)
    artifacts.append(record(beam/'exit_reference.beam'))
    for profile, flags, flavor in [('jit', [], 'jit'), ('debug-emu', ['-emu_type','debug','-emu_flavor','emu'], 'emu')]:
        c,out,err = invoke('beam-'+profile,[otp/'bin/erl',*flags,'-noshell','-pa',beam,'-s','exit_reference','run'])
        assert c['exit_code'] == 0 and not c['timed_out'] and not err, (out,err)
        reference = json.loads(out)
        assert reference['otp_release'] == '29' and reference['emu_flavor'] == flavor
        assert reference['cases'] == oracles.reference()
        observations.append({'kind':'beam_reference','command':c['name'],'profile':profile,**reference})
    c, out, err = invoke('beam-timed-compile', [otp/'bin/erlc','-Werror','+debug_info','-o',beam,PROJECT/'timed_read_reference.erl'])
    assert c['exit_code'] == 0 and not c['timed_out'], (out,err)
    artifacts.append(record(beam/'timed_read_reference.beam'))
    for profile, flags, flavor in [('jit', [], 'jit'), ('debug-emu', ['-emu_type','debug','-emu_flavor','emu'], 'emu')]:
        c,out,err = invoke('beam-timed-'+profile,[otp/'bin/erl',*flags,'-noshell','-pa',beam,'-s','timed_read_reference','run'])
        assert c['exit_code'] == 0 and not c['timed_out'] and not err, (out,err)
        timed = json.loads(out)
        assert timed['otp_release'] == '29' and timed['emu_flavor'] == flavor
        assert timed['cases'] == oracles.timed_reference()
        observations.append({'kind':'beam_timed_reference','command':c['name'],'profile':profile,**timed})
    common = ['--offline', '--locked', '--manifest-path', PROJECT/'Cargo.toml']
    build = [TOOLCHAIN/'cargo', 'build', *common, '-p', 'bream-timed-consumer']
    negatives = sorted((PROJECT/'consumer/examples').glob('reject_*.rs'))
    diagnostic = {
        'reject_timed_read_missing_timeout':'E0004', 'reject_timed_read_duplicate_owner':'E0382', 'reject_stream_missing_busy':'E0004', 'reject_stream_duplicate_owner':'E0382', 'reject_read_missing_arm':'E0004', 'reject_read_duplicate_owner':'E0382', 'reject_exit_missing_arm':'E0004', 'reject_future':'mismatched types', 'reject_closure_spawn':'mismatched types',
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
        c, out, err = invoke('unit-'+profile, [TOOLCHAIN/'cargo','test',*common,'-p','bream-timed-runtime','--lib',*flags])
        assert c['exit_code'] == 0 and not c['timed_out'] and '153 passed; 0 failed' in out, (out, err)
        observations.append({'kind':'unit_tests','command':c['name'],'profile':profile,'tests':153,'passed':True})
        c, out, err = invoke('build-'+profile, build+flags)
        assert c['exit_code'] == 0 and not c['timed_out'], err
        binary = TARGET/profile/'bream-timed-consumer'; artifacts.append(record(binary))
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
        for allowance in protocol['allowances']:
            c, out, err = invoke(f'replacement-{profile}-{allowance}', [binary,'replacement',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'], (out, err)
            assert 'index out of bounds' in err, err
            row = json.loads(out)
            assert row['passed'] and row['speed_tests'] is False and row['registry_capacity'] == 3
            assert row['transcripts'] == [[[10,1,626271,0],[11,1,626271,2],[20,1,626271,3],[30,111,626272,0],[40,112,626273,0]]]*2
            assert row['ownership'] == [[[4,0,0,1,6,5],[0,4,1,0,6,6]]]*2 + [[[4,0,0,1,25,24],[0,4,1,0,25,25]]]*2
            assert row['terminal_rows'] == [[0,2,22,3],[1,2,22,3]]
            observations.append({'command':c['name'],'profile':profile,**row})
        for allowance in protocol['allowances']:
            c, out, err = invoke(f'exits-{profile}-{allowance}', [binary,'exits',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'] and not err, (out,err)
            row = json.loads(out)
            oracles.check_exit(row)
            beam_rows = [[r[k] for k in ['mode','reason','trap','outcome','observed_reason','returned']] for r in reference['cases'] if r['kind'] == 'matrix']
            assert row['matrix'] == beam_rows
            observations.append({'command':c['name'],'profile':profile,**row})
        for allowance in protocol['allowances']:
            c, out, err = invoke(f'reads-{profile}-{allowance}', [binary,'reads',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'] and not err, (out,err)
            row = json.loads(out)
            oracles.check_read(row)
            observations.append({'command':c['name'],'profile':profile,**row})
        for allowance in protocol['allowances']:
            c, out, err = invoke(f'streams-{profile}-{allowance}', [binary,'streams',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'] and not err, (out,err)
            row = json.loads(out)
            oracles.check_stream(row)
            observations.append({'command':c['name'],'profile':profile,**row})
        for allowance in protocol['allowances']:
            c,out,err = invoke(f'timed-reads-{profile}-{allowance}',[binary,'timed-reads',allowance])
            assert c['exit_code'] == 0 and not c['timed_out'] and not err, (out,err)
            row = json.loads(out)
            oracles.check_timed_read(row)
            observations.append({'command':c['name'],'profile':profile,**row})
        for example in ['shadow_paths','shadow_structured','shadow_mailbox','shadow_timeout','shadow_monitor','shadow_link','shadow_spawn','shadow_shared','shadow_exit','shadow_read','shadow_stream','shadow_timed_read']:
            c, out, err = invoke(example+'-build-'+profile, build+flags+['--example',example])
            assert c['exit_code'] == 0 and not c['timed_out'], err
            binary = TARGET/profile/'examples'/example; artifacts.append(record(binary))
            c, out, err = invoke(example+'-execute-'+profile, [binary])
            assert c['exit_code'] == 0 and out == example.upper()+'_PASSED\n' and not err, (out, err)
        for fixture in negatives:
            c, out, err = invoke('reject-'+profile+'-'+fixture.stem, build+flags+['--example',fixture.stem])
            default = 'bream structured subset:' if fixture.stem.startswith(('reject_struct_', 'reject_timeout_', 'reject_monitor_', 'reject_link_', 'reject_spawn_', 'reject_shared_', 'reject_exit_', 'reject_read_', 'reject_stream_', 'reject_timed_read_')) or fixture.stem in new_grammar else 'bream milestone subset:'
            required = diagnostic.get(fixture.stem, default)
            assert c['exit_code'] != 0 and not c['timed_out'] and required in err, (fixture.name, err)
            observations.append({'kind':'rejection','command':c['name'],'profile':profile,'fixture':fixture.stem,'diagnostic':required,'passed':True})
except Exception as error:
    errors.append(repr(error))
result = {
    'status':'failed' if errors else 'passed', 'errors':errors, 'protocol':record(RUN/'protocol.json'),
    'inputs':inputs,'tools':[record(TOOLCHAIN/t) for t in ['rustc','cargo']], 'artifacts':artifacts,
    'commands':commands,'observations':observations,'environment_overrides':OVERRIDES,
    'correctness_only':True, 'speed_tests':False,'performance_comparisons':False,'new_beam_execution':True,'production_runtime':False,
}
(RUN/'results.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'status':result['status'],'commands':len(commands),'observations':len(observations),'inputs':len(inputs),'speed_tests':False,'errors':errors},indent=2))
raise SystemExit(bool(errors))
