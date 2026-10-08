# Mailbox and receive: bounded edge-case audit

**We have not established behavior in every case.** This audit resolves sixteen ordinary local scenarios against Erlang/OTP **OTP-29.1.1**, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`, inspected 2026-10-07. It separates reference semantics from source mechanics and remaining race analysis. None of these cases is yet a passing Bream conformance result. **No tests, builds or experiments were run for this audit**; upstream tests were read as evidence of intended regressions.

Scope: ordinary local PID sends, ordinary-priority mailboxes, application messages and ordinary runtime notifications. Priority messages, distribution, aliases and registered-name addressing are outside this initial slice. They are deliberate scope exclusions, not evidence that their behavior is unknown to BEAM.

Status labels: **D** = documented outcome; **I** = pinned implementation detail; **R** = concurrent outcomes must be represented without inventing a universal winner; **U** = this audit has not completed the necessary trace. All source links below are immutable.

## Sixteen cases

1. **One sender sends two messages to one receiver — D.** Signals preserve sender-to-destination order. With ordinary priority, delivered messages enter the mailbox in that order. Selective receive may consume the second first if the first does not match: delivery order is not processing order. This guarantee also crosses signal kinds, such as an ordinary message followed by an exit signal. [Signal ordering][order]; [ordinary enqueue order][enqueue].

2. **Independent senders race — D/R.** There is no documented global ordering across independent senders. A conformance case must permit interleavings consistent with each sender's own sequence. Sender-side timestamps or completion of two unrelated send calls do not establish a receiver-side total order. Avoid tests that designate one sender as the winner merely because one run did so. [Signal ordering][order].

3. **Send to a dead PID, or race receiver death — D/I/R.** PID send does not report a missing receiver as an error or acknowledge consumption. The local lookup returns normally when absent; the locked queue path also drops messages after discovering an exiting receiver. A live lookup therefore does not promise delivery. This differs from an unregistered local name, which errors. Bream must dispose of moved owned payloads safely on the dead-recipient path. [Send contract][send]; [lookup][lookup]; [exiting receiver][drop].

4. **Send to self — D/I.** This remains mailbox messaging; it does not synchronously invoke a handler or bypass selective receive. The ordinary on-heap self-send path avoids copying the term but still calls `erts_queue_proc_message`. That optimization relies on BEAM ownership and is not permission to keep an unrestricted Rust borrow across process operations. Self-sent messages retain the same documented ordering rules. [Send contract][send]; [self-send source][self].

5. **Several messages and clauses match — D.** Search messages from the mailbox start; for each message, try clauses from top to bottom. An older message matching clause two beats a newer message matching clause one. Once selected, remove only that message and execute its body. This is a message-major search, not a separate queue or scan for each clause. [Receive contract][receive].

6. **Pattern matches but guard is false or invalid — D.** A clause requires its guard sequence to succeed. Guard alternatives separated by semicolons are tried in order; invalid arguments in documented guard expressions cause guard failure, not ordinary expression failure escaping from the receive. Failure of all guards leaves that clause unsuccessful, allowing later clauses/messages. This does not make a Rust panic equivalent to a BEAM guard failure. [Receive contract][receive]; [guard sequences][guards]; [guard failure][guard-failure].

7. **Messages never match this receive — D.** They remain unchanged, retaining their relative order; they are neither discarded nor reported as errors. Without a finite timeout the receive may remain suspended indefinitely. The statement that receive does not fail for lack of a match does not promise that its selected body cannot raise an exception. [Receive contract][receive].

8. **Scan exhausts its scheduling allowance, then resumes — I.** Failed matching advances the saved position and spends a reduction. Receive execution can schedule out and resume with that position preserved. In the inspected ordinary path, previously rejected messages need not be rescanned each turn. This is a resumable scan optimization, not a language guarantee about predicate invocation counts or arbitrary stateful predicates. [Scan scheduling][scan]; [advance and charge][advance].

9. **Next receive follows a match or timeout — I supporting D.** Selection unlinks the chosen message, resets the saved scan position and cancels the timer. Timeout also resets the position. Thus a later receive with different clauses can select an older previously unmatched message. Compiler-proved reference markers can skip an irrelevant prefix without changing selection semantics; their full lifecycle is not qualified here. [Selection cleanup][selection]; [timeout reset][timeout-reset]; [reference optimization][reference].

10. **Zero timeout with a large unmatched prefix — D/I.** `after 0` still searches for a matching queued message. If none matches, it executes the timeout body after searching; this can take multiple scheduling turns. It is not a constant-time emptiness check or a wall-clock zero-duration promise. Racing new arrivals require an execution-level analysis, not an assumed atomic snapshot at source-expression entry. [Timeout contract and warning][timeout-doc]; [scan scheduling][scan].

11. **Infinite timeout — D/I.** `infinity` means indefinite wait for a matching message, equivalent to no timeout. It is not a large finite duration that eventually overflows. The compiler can lower a literal `infinity` to `wait`; the dynamic helper records the no-rearm state without installing a finite timer. Kill and other runtime signals remain effective while application receive waits. [Timeout contract][timeout-doc]; [compiler infinity][infinity]; [wait helper][wait].

12. **Unmatched arrivals repeatedly wake a finite receive — I.** The inspected wait instructions arm the receive timer only when neither timer-state flag is set; unmatched wakeups do not restart the duration. Initial scanning precedes this wait/arm path. Preserve that observed scan/wait behavior, while avoiding a stronger claim that every source-level receive begins an independently measured deadline at expression entry. [Wait helper][wait]; [lowered receive loop][lowering].

13. **Matching arrival races timeout — R/U.** Matching, message publication, timer expiry and scheduler continuation are distinct events. A timer callback atomically changes the matching timer pointer to `TIMEDOUT`; selection cancels the timer through atomic exchange; a scheduled process with timeout recorded is directed to its timeout continuation. These facts do not establish one winner from application timestamps. Complete allowed interleavings across publication, scanning and rescheduling remain unaudited. [Expiry CAS][expiry]; [cancellation][cancel]; [scheduler continuation][continuation]; [selection cleanup][selection].

14. **Invalid timeout or exceptional timeout expression — D/I/U.** Documented values are integers `0..4294967295` or `infinity`. The wait helper reports `timeout_value` on an invalid value and resets the cursor; upstream regressions verify that catching this error does not hide old messages. Timeout-expression evaluation and value validation are distinct: lowering evaluates an effectful expression outside the loop, but the wait primitive follows scan exhaustion. Do not claim every malformed timeout errors before an already-available match; all optimized variants were not audited. [Timeout contract][timeout-doc]; [lowering][lowering]; [wait helper][wait]; [reset regression][reset-test].

15. **Producer publishes as receiver goes to sleep — I/U.** The empty-scan helper returns holding the receive queue lock; the wait path clears ACTIVE and releases that lock. The producer's notification then schedules inactive receivers. The optional buffered path explicitly synchronizes its first-buffer notification against that transition. This explains lost-wakeup prevention, but does not prove every buffer/flush interleaving or a proposed Rust queue. [Empty-scan handoff][handoff]; [wait helper][wait]; [buffer handshake][buffer].

16. **Kill while waiting, possibly alongside messages — D/I/R.** Direct untrappable kill is a runtime signal; it does not require the receive clauses to select an application notification. Its handler changes the reason to `killed` and sets exiting state; thereafter no more Erlang code executes. Messages can be consumed before kill takes effect, but cannot justify execution afterward. Earlier delivery does not require earlier application consumption. The upstream message/exit ordering test explicitly allows preceding messages not to be consumed. [Exit action contract][exit-doc]; [kill handling][kill]; [receive exit path][exit-path]; [ordering regression][order-test].

## Corrections and limits of the earlier mailbox report

Report [12](12-beam-mailboxes-source.md) should not be read as a complete semantic specification. Its signal-budget description has been corrected: the handler adds an operation's weighted cost, then checks the loop limit, and rounds returned accounting. A nominal maximum is not a precise bound on performed work or latency. Direct evidence is [weighted exit cost][weighted], [post-operation check][postcheck] and [returned accounting][accounting].

Its cursor language applies to the inspected ordinary scan. It does not admit arbitrary Rust closures as guards, promise callback invocation counts, or cover priority-marker interactions. Its timer discussion describes where ordinary generated execution arms a timer; it does not settle every race or create a stronger deadline contract.

## Highest-priority unfinished work

- **Race outcome model:** enumerate publication, timer CAS, matching cancellation, scheduling and exiting transitions together. Separate “message was sent” from “received into the mailbox” and “selected.” Derive allowed traces before creating deterministic boundary tests; do not designate one wall-clock tie winner.
- **Receive-state generations:** trace cancellation and stale timer events across match, timeout, exception and process death. Bream must prevent a previous receive's event from completing a later receive or waking a recycled process incarnation.
- **Guard and exception admission:** finish compiler lowering coverage for approved patterns/guards, exceptional timeout expressions and invalid values with queued matches. Determine enforceable Rust equivalents without importing arbitrary predicate side effects or destructors into selection.
- **Ownership during kill:** follow all queue, fragment and pending-signal ownership transitions through termination. The semantic fact “no more user execution” is not a proof that every resource was reclaimed exactly once or that cleanup is latency-bounded.
- **Qualification:** port bounded scenarios, including upstream ordering and save-reset regressions, then inspect outcomes and forbidden traces. Reading test source is not executing those tests, and a passing sample run is not a concurrency proof.

Useful local entry points: [receive instructions](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/emu/msg_instrs.tab:44), [timer expiry](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_hl_timer.c:692), [wait helper](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/jit/beam_jit_common.cpp:1192), [save-reset regression](/home/bat/projects/bream/upstream/otp/erts/emulator/test/receive_SUITE.erl:694).

[order]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L495-L512
[enqueue]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L345-L359
[send]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/expressions.md#L647-L666
[lookup]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L2458-L2480
[drop]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.c#L405-L426
[self]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.c#L821-L874
[receive]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/expressions.md#L680-L697
[guards]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/expressions.md#L2238-L2257
[guard-failure]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/expressions.md#L2331-L2334
[scan]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/emu/msg_instrs.tab#L44-L112
[advance]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/emu/msg_instrs.tab#L249-L261
[selection]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/beam_jit_common.cpp#L1167-L1171
[timeout-reset]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/beam_jit_common.cpp#L1242-L1255
[reference]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/expressions.md#L699-L713
[timeout-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/expressions.md#L729-L763
[infinity]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/compiler/src/beam_ssa_codegen.erl#L1454-L1464
[wait]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/beam_jit_common.cpp#L1188-L1239
[lowering]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/compiler/src/v3_core.erl#L4337-L4406
[expiry]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L691-L706
[cancel]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2831-L2845
[continuation]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10451-L10464
[reset-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/receive_SUITE.erl#L694-L757
[handoff]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7771-L7798
[buffer]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L10381-L10418
[exit-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L305-L310
[kill]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4738-L4782
[exit-path]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7820-L7846
[order-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/signal_SUITE.erl#L155-L185
[weighted]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6336-L6352
[postcheck]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6968-L6971
[accounting]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7160-L7168
