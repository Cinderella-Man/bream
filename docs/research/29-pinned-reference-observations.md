# Pinned BEAM process observations: predictions tested

**Executed 2026-10-07.** This pass built the pinned OTP `29.1.1` source, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`, and ran targeted local process fixtures. The runtime identifies itself as **ERTS 17.1**. Earlier reports' selected source predictions now have bounded executable evidence. This is not the upstream test suite, exhaustive race exploration, or a Bream implementation/conformance result.

The main new result is a reproduced assertion failure in an optimized priority-receive path on the debug interpreter, with a passing unoptimized control. Other fixtures establish concrete behavior for synthetic demonitor flush, paused timer references, dead-target suspension acknowledgments, notification ordering, hibernation arity and numeric GC settings. Source and observations must remain distinct from the intended compatibility contract.

## Build identity and reproducibility

The original `upstream/otp` checkout remains unchanged. A separate ignored local clone, `upstream/otp-reference-build`, supplied both executables. The installed Erlang was 29.0.5/ERTS 17.0.5 and was not used as the semantic reference. [Build provenance, binary hashes and commands](../../evidence/process-protocol-review/source-build.json).

The first full application build stopped in the optional debugger application because disabling wx left an undefined `wx_object` behaviour under warnings-as-errors. The subsequent **tiny/core target completed**, followed by a debug interpreter build. This is a qualified core reference build, not a successful build of every OTP application. Crypto, SSL, SSH, Java, wx and ODBC support were disabled. No system installation, source patch or distributed/network test was performed.

| Profile | Emulator | Normal scheduler workers | Evidence |
|---|---|---:|---|
| jit-1 | Optimized x86 JIT | 1 | [Manifest](../../evidence/process-protocol-review/jit-1/run-manifest.json), [observations](../../evidence/process-protocol-review/jit-1/observations.json) |
| jit-4 | Optimized x86 JIT | 4 | [Manifest](../../evidence/process-protocol-review/jit-4/run-manifest.json), [observations](../../evidence/process-protocol-review/jit-4/observations.json) |
| debug-emu-4 | Debug interpreter, assertions and lock checking | 4 | [Manifest](../../evidence/process-protocol-review/debug-emu-4/run-manifest.json), [observations](../../evidence/process-protocol-review/debug-emu-4/observations.json) |

Each profile completed 11 fixture families and three separately isolated hibernation boundaries. The ordering family used 100 samples for each of four option combinations: 1,200 samples across profiles. Six additional marker checks compare optimized and `no_recv_opt` code; five complete and the optimized debug-interpreter check aborts at the predicted assertion. Counts describe these fixtures, not coverage percentages.

Replay after building the recorded source targets:

```bash
cd /home/bat/projects/bream
python3 evidence/process-protocol-review/run_reference.py --profile jit-1
python3 evidence/process-protocol-review/run_reference.py --profile jit-4
python3 evidence/process-protocol-review/run_reference.py --profile debug-emu-4
```

The [runner](../../evidence/process-protocol-review/run_reference.py) records each command, exit code, timeout status, raw output and hashes. It disables core dumps in these disposable VMs. The debug marker abort is a retained finding, not a green test pass. [Combined observations](../../evidence/process-protocol-review/observations-summary.json).

Compiled output paths were initially shared across profiles. Final validation separately retained the last profile's artifacts and rebuilt the earlier two with their original compiler commands and working directories; all match their original recorded hashes byte-for-byte. [Recovery provenance](../../evidence/process-protocol-review/compiled-artifact-recovery.json) records this distinction. Original behavioral observations and manifests were left intact.

## 1. Synthetic flush is not unconditionally the documented expansion

With a live target and active ordinary monitor, the fixture self-sends `{synthetic, Ref, a, b, c}`. Both `[flush]` and `[flush, info]` return `true` and leave that tuple queued. A second call with `[flush, info]`, after the monitor is absent, returns `false` and removes it. The documented expanded expression—plain demonitor followed by a matching receive—removes the tuple immediately. A separate real-DOWN fixture establishes that an already-materialized genuine notification is removed by flush.

All three VM profiles agree. This confirms the specific prediction in [report 16](16-lifecycle-edge-cases.md); it does not negate suppression of future genuine DOWN after cancellation, or imply that flush drains every matching tuple. The active ordinary-monitor C path skips the receive helper, while the absent-origin path uses it. [C branch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L598-L654), [documented equivalence](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L2074-L2089).

**Bream:** the typed envelope/look-alike mapping remains open. Evidence now answers what this pinned implementation does for the tested tuple; it does not decide which Rust values are constructible or erase the documentation discrepancy. Related cases: MSG-01.C07, LIFE-02.C12 and REF-01/02.

## 2. Paused timer lookup identity can disappear while delivery survives

Separate cohorts avoid confounding cancellation with read-only observation. Four 1,000 ms timers are created for another process; all must still have more than 500 ms remaining before suspension, and suspension must occur within 500 ms of setup. Separate timers are created while suspended. Post-resume observations must also complete within 500 ms. These are the original semantic setup checks against ordinary pre-observation expiry. Recorded elapsed intervals were removed with the user-requested performance-evidence reset.

Across all profiles:

- Original references return `false` from read while paused and after resume, including untouched read-only timers.
- Cancel on separate references returns `false` while paused or after resume.
- Timers created while suspended show the same read/cancel behavior.
- All six payloads still arrive. The `start_timer` payloads retain their original visible reference; `send_after` retains its ordinary payload.

This supports [report 19](19-process-signals-timers-deep-dive.md)'s explanation: pause saves payload/remaining time, cancels the old timer entry, and resume creates a new entry/reference without replacing the old reference embedded in a saved timeout tuple. It establishes the observed identity discontinuity, not every suspension/expiry race or intended future contract. [Pause storage](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L994-L1022), [reconstruction](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2948-L3027).

A separate fixture receives the creator's DOWN before selecting the forwarded timeout. This establishes that delivery was observed after death was observed, but does not establish that the timer was still active at death: an expired timeout could already be queued. The stronger creator-versus-recipient lifetime rule remains source-supported in report 19, not independently proven by this fixture. Explicit suspension remains different from normal scheduling preemption, which must not pause deadlines. **Bream:** retain TIME-01.C02/C06/C07 as explicit compatibility/identity work; do not accidentally implement false cancellation acknowledgments or copy a reference discontinuity without resolving the API contract.

## 3. A dead-target tagged suspend reply has a fourth observed value

For a PID whose DOWN has already been received, tagged asynchronous `suspend_process` returns `true` and sends `{Tag, badarg}`. Untagged asynchronous call returns `true`; the synchronous call raises an error-class `badarg`. All profiles agree.

This reaches the enqueue-failure branch, where the async/sync selection differs from the exiting-target consumer's selection. The documented tagged state list contains `suspended`, `not_suspended` and `exited`; its general failure list also mentions dead-target badarg but does not explain this fourth tagged payload. [Producer fallback](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L3214-L3242), [consumer](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L5516-L5527), [documentation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L6304-L6355).

The narrow synchronous disappearance-after-send race remains unexecuted. [Report 26](26-suspension-system-work-protocol.md) traces why it differs from early synchronous rejection. **Bream:** explicit suspension is later scope; EXEC-08.C18 now has observations, while C19 and pending-dirty acknowledgment products remain research cases. Do not make a three-variant result type from the documentation alone without resolving this branch.

## 4. Notification mailbox order depends on priority reception

The observer traps exits, establishes a link and monitor, self-enqueues `before`, then permits the target to exit abnormally. It waits until both EXIT and DOWN are materialized before consuming all three items. That barrier matters: otherwise immediate consumption of `before` would obscure later priority placement.

| Link receives priority exits | Monitor receives priority DOWN | Observed mailbox order, every sample |
|---|---|---|
| No | No | before → EXIT → DOWN |
| No | Yes | DOWN → before → EXIT |
| Yes | No | EXIT → before → DOWN |
| Yes | Yes | EXIT → DOWN → before |

These 1,200 observations support the phase-order/source prediction for this local setup. They are not a global guarantee for all spawn options, independent senders, tag/alias combinations, remote nodes or when the observer itself dies. Priority changes message placement after ordered signal handling; it does not reorder the underlying signal protocol. [Priority contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L376-L389), [lifecycle phase trace](20-process-lifecycle-deep-dive.md).

The same runs confirm absent-local-target link raises catchable `error:noproc` when untrapped, but returns `true` and produces EXIT/noproc when trapped. **Bream:** ordinary local cases have stronger oracles now; option/race products remain explicit in LIFE-01/03/07 and MSG-06.

## 5. Priority marker optimization reproduces a debug assertion

The [small reproducer](../../evidence/process-protocol-review/priority_marker_probe.erl) leaves an old sentinel queued, creates a fresh reference and priority alias, receives an ordinary barrier sent after the priority reply, deactivates the final alias, and then receives the reference-bearing reply. It checks that priority reception is disabled while the previously accepted reply remains selectable.

Disassembly verifies the target `probe/0` function reserves a marker before `make_ref`, binds it, and uses it after `unalias`. Compiling with `no_recv_opt` removes marker instructions. This avoids confusing an optimization's absence with successful exercise of its runtime path. [Optimized disassembly](../../evidence/process-protocol-review/jit-1/disassembly-normal.log), [unoptimized control](../../evidence/process-protocol-review/jit-1/disassembly-no_recv_opt.log).

| Configuration | Optimized fixture | `no_recv_opt` fixture |
|---|---|---|
| Release JIT, one worker | Reply received; sentinel retained | Same |
| Release JIT, four workers | Reply received; sentinel retained | Same |
| Debug interpreter, four workers | SIGABRT, assertion at `erl_proc_sig_queue.h:2190` | Reply received; sentinel retained |

[Failure log](../../evidence/process-protocol-review/debug-emu-4/priority-marker-normal.log), [same-debug-VM control](../../evidence/process-protocol-review/debug-emu-4/priority-marker-no_recv_opt.log).

The source asserts active priority reception when redirecting an optimized marker into the priority prefix, while last-capability removal can clear that flag but retain the queued prefix. [Report 28](28-selective-receive-marker-protocol.md) traces the mismatch. This is a **reproduced failure on the specified debug build**, stronger evidence than the earlier hypothesis. It does not demonstrate release-JIT message loss, establish every backend's behavior, or make a VM abort part of Bream's intended semantics. No upstream patch or issue was submitted.

**Bream:** preserve already accepted messages after capability removal; optimization needs equivalence checks plus internal-state invariants. MSG-06.C07 and MSG-07.C03 now have a concrete failing reference variant and passing controls.

## 6. Hibernation, timeout validation and numeric boundaries

For `hibernate(erlang, length, Args)` with deliberately nonexistent target arities:

| Argument count | Observed phase/result in all profiles |
|---:|---|
| 255 | Old catch context discarded; process exits with uncaught `undef` for length/255 |
| 256 | Old catch context discarded; process exits with uncaught `undef` for length/256 |
| 257 | Original catch catches error-class `system_limit` |

Each runs in a separate VM/process. This confirms the exact-256 early-check gap recorded in [report 18](18-process-execution-memory-deep-dive.md); it is not a claim that length/256 is callable or hibernation generally succeeds at that arity. Improper-tail combinations and other architectures remain unexecuted. [Validation/reset branch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1366-L1427).

The dynamic invalid receive timeout fixture selects a queued matching message; when only an unmatched message is queued it raises `error:timeout_value`. A following receive recovers that retained unmatched message. This checks wait-path validation and cursor recovery for this dynamic form, not every constant-folded compiler path or arrival/expiry race.

`fullsweep_after` round-trips 0, 1 and 65,535, but 65,536 is observed as 0 and 65,537 as 1. This confirms the stored-field-width concern, not a chosen Rust memory policy or hard memory limit. [Setter](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L2101-L2112). Related cases remain MEM-01.C03/C13 and MSG-02.C06/MSG-03.C04.

## What this closes, and what it does not

The tested source predictions now have versioned observations and reproducible controls. The independent source traces also deepen suspension ownership, process lookup/reclamation and receive-marker state. None qualifies Bream's native code admission, automatic suspension representation, safe Rust resource reclamation, complete races or performance bounds.

Clear supported BEAM behavior remains the default. When documentation and the pinned implementation disagree, record both and isolate the affected compatibility case. An implementation crash is evidence to investigate, not behavior to emulate in a safe Rust API. The next useful work is controlled interleaving/ownership models for these protocols and a useful external-crate Rust admission experiment; collecting more API names alone will not resolve those gates.
