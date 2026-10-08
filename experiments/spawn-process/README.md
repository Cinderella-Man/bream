# Atomic local spawn with initial relationships

**Passed for the restricted local single-worker profile, 2026-10-08.** This eighth bounded milestone extends [local links and trapped exits](../link-process/README.md) with local child creation that installs its initial link, monitor, or both before dispatch. It is a native Rust experiment for the restricted single-worker profile, with an injected logical clock. [Final cohort](../../evidence/spawn-process/runs/20261008-spawn-02/results.json), [read-only audit](../../evidence/spawn-process/validation.json), [milestone tracker](../../docs/milestones.md).

**No speed tests or performance comparisons may run on this machine.** Source inspection, builds and bounded correctness controls are the available evidence. Logical events and injected ticks establish semantic order only. Watchdog elapsed values are not performance evidence. Performance qualification requires a separately authorized suitable environment. [Persistent instructions](../../AGENTS.md).

## Checked application boundary

The separate consumer uses the [checked grammar](macros/src/structured.rs) and [runtime context](runtime/src/lib.rs). These operations are experimental authored syntax, not a finalized library interface.

| Authored operation | Meaning |
|---|---|
| `spawn_link(child, words, tag, argument)` | Move the owned input into a new local child and install a bidirectional link to the caller before dispatch. Return its `Pid`. |
| `spawn_monitor(child, words, tag, argument)` | Move the input into a child and install a one-way monitor from the caller before dispatch. Return a sealed `Spawned` value. |
| `spawn_link_monitor(child, words, tag, argument)` | Install both relationships in the same publication operation. Return a sealed `Spawned` value. |
| `spawned_pid(spawned)` / `spawned_ref(spawned)` | Read the child's complete process identity and its initial monitor reference. |

`child` must name a checked function declared in the same module, with no explicit parameters and unit output. It is selected statically; the boundary does not admit arbitrary closures, external functions, callbacks or native futures. The combined helper/spawn graph rejects cycles and depth beyond eight functions. The grammar generates its sealed `Program` factory. The remaining operands are one moved `Words` value and two `u64` values; they are evaluated once in source order before the runtime call.

The child receives those values through the existing `owned_input()`, `input_tag()` and `input_argument()` operations. Its `input_target()` is the spawning origin in this prototype. That explicit input mapping is Bream's test interface, not an inherited BEAM process flag. A child starts with exit trapping disabled regardless of its parent's flag. Existing link, monitor, mailbox and finite receive operations remain available.

Authored spawn failure terminates the current checked process through the existing restricted uncaught-failure path. General exception catching and a typed recoverable authored spawn operation are outside this slice. An immediately failing child is different: successful creation still produces a real child identity and its committed failure is observed through the installed relationships.

## Trusted host controls and publication

[Runtime publication](runtime/src/lifecycle.rs) exposes these trusted host controls:

| Host operation | Successful result |
|---|---|
| `RuntimeClient::spawn_link(origin, program, input)` | `ProcessHandle` |
| `RuntimeClient::spawn_monitor(origin, program, input)` | `(ProcessHandle, MonitorRef)` |
| `RuntimeClient::spawn_link_monitor(origin, program, input)` | `(ProcessHandle, MonitorRef)` |

The host supplies an explicit origin identity and may race these controls with other host operations. The checked application operation always uses its own identity. Host controls are not evidence that an actor can continue calling functions after processing its own fatal exit.

Publication validates the origin's full runtime, slot and generation identity under the same registry guard used by stop and shutdown. It reserves the child's identity, installs the requested relationship metadata and sends the child to the worker before releasing that guard. No application poll can precede relationship installation. The child may run or stop before the caller observes the successful return; a returned identity is not a lease on a live process.

An accepted but unhandled kill request does not by itself remove the origin from the registry. Publication can still win before actual stop; the subsequent committed stop reason then follows the installed link. If stop removes the origin first, publication rejects it. This separates stop requests from actual stop publication and avoids inventing a stronger cancellation rule than the pinned reference. A normal linked parent exit leaves a default, nontrapping child alive; an abnormal linked exit can stop it. A monitor alone never propagates parent death into the child.

Host rejection distinguishes `InputCapacity`, `OriginNotLive`, `Capacity`, `Closed`, `MonitorExhausted` and `Disconnected`. A stale or foreign origin is not a distributed spawn target. Capacity, liveness and closed-runtime rejection do not leave an initial relationship. Monitor-reference exhaustion rolls back the reserved child identity before dispatch. A disconnected dispatch removes the new link and monitor, removes the reserved identity and closes further publication. Prepared application frames are destroyed after releasing the registry guard. Rejection does not send a child Exit or Down notification.

The host `Input` capacity limit remains 8192 words of allocated capacity. Authored child input is already an admitted `OwnedWords` buffer. Moving it preserves its original allocation receipt: the child does not copy the payload or reattribute its storage to new child diagnostics. The receipt can outlive the producer without retaining the producer's application frame. Tests must examine the original receipt when checking that transferred input is released exactly once.

Initial relationships reuse the prior weak endpoint tables and full identities. Monitors allocate before links because reference exhaustion is fallible. No spawn-then-monitor or spawn-then-link window is introduced. Cleanup retains the prior limit of eight combined link and monitor items per retirement step, with links before monitors. An ordinary trapping parent observing both relationships receives the child's Exit before its Down; application messages already sent by that child precede both in the shared mailbox. These statements concern notification order, not guaranteed consumption before fatal termination or final reclamation.

## Pinned reference obligations

The clean OTP reference is `ad05823719d77c8faee87348ea39513d4e2f99c5`. This audit inspected local synchronous spawn branches without executing BEAM.

| Obligation | Local reference anchor |
|---|---|
| `spawn_link` creates the relationship atomically with process creation | [Documented contract](../../upstream/otp/erts/preloaded/src/erlang.erl:6160), [link spawn BIF](../../upstream/otp/erts/emulator/beam/bif.c:1003) |
| `spawn_monitor` uses the monitored spawn operation; link and monitor flags combine independently | [Monitor wrapper](../../upstream/otp/erts/preloaded/src/erlang.erl:10657), [option parsing](../../upstream/otp/erts/emulator/beam/erl_process.c:12256) |
| The running parent retains its main lock and acquires its minor locks during local creation | [Main-lock contract](../../upstream/otp/erts/emulator/beam/erl_process_lock.h:112), [parent setup](../../upstream/otp/erts/emulator/beam/erl_process.c:12471) |
| Both local link directions and both monitor halves exist before scheduling the child | [Child allocation with locks](../../upstream/otp/erts/emulator/beam/erl_process.c:12535), [link insertion](../../upstream/otp/erts/emulator/beam/erl_process.c:12911), [monitor insertion](../../upstream/otp/erts/emulator/beam/erl_process.c:12932), [scheduling](../../upstream/otp/erts/emulator/beam/erl_process.c:13121) |
| Monitored synchronous spawn returns the child's PID and the created monitor reference | [Return construction](../../upstream/otp/erts/emulator/beam/bif.c:1058) |
| Trap-exit state is not inherited from the parent | [Fresh flags](../../upstream/otp/erts/emulator/beam/erl_process.c:12444), [child flag assignment](../../upstream/otp/erts/emulator/beam/erl_process.c:12552) |
| Exit processing cannot dismantle the running parent's relationship tables midway through local spawn | [Signal-handling lock assertion](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6242), [fatal classification](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:4782), [exit transition locks](../../upstream/otp/erts/emulator/beam/erl_process.c:14190) |
| Recoverable process allocation or table failure happens before link and monitor insertion | [Allocation and table rollback](../../upstream/otp/erts/emulator/beam/erl_process.c:12185), [system-limit branch](../../upstream/otp/erts/emulator/beam/erl_process.c:12535) |
| Failure to execute a child's start function occurs in the child, rather than turning completed creation into spawn rejection | [Child execution contract](../../upstream/otp/erts/preloaded/src/erlang.erl:6134) |
| Ordinary link notifications precede monitor cleanup for the same exiting process | [Cleanup phases](../../upstream/otp/erts/emulator/beam/erl_process.c:14545) |

The inference about a kill arriving during the BIF follows from these locking and signal-handling branches: its signal can be pending while local publication completes, and later handling propagates the committed reason across the installed relationship. It is not a guarantee that every concurrent host operation has the same internal ordering as a BEAM BIF.

Reference allocation can precede checks that fail; the pinned code creates the monitor reference before argument and process-capacity validation. Gapless references are therefore not an obligation. The Rust rollback path is also narrower than a general allocation transaction: the pinned local code has recoverable validation and process-table failures before relationship insertion, but does not recover every later heap allocation failure. This experiment's disconnected channel and finite monitor-reference errors are explicit native boundaries. It does not promise recovery from allocator abort, an invalid unsafe program factory or arbitrary destructor failure.

The [BEAM-default decision](../../docs/adr/0010-beam-observable-semantics-by-default.md), [exit/trapping decision](../../docs/adr/0006-beam-exits-and-trapping.md) and [stop-before-notification decision](../../docs/adr/0009-death-and-reclamation-observation.md) supply the existing semantic direction; this slice introduces no new preference decision.

## Qualification and Rust opportunities

The final [correctness cohort](../../evidence/spawn-process/README.md) passed **311 expected command outcomes**: **83 unit tests per debug/release profile**, six public spawn configurations, inherited mailbox/timeout/monitor/link controls, **248 expected compilation rejections** and **fourteen positive shadowing executions**, plus builds. There are **96 spawn/parent-kill and 96 spawn/shutdown races** with allowed-outcome assertions. No fixed race winner or exhaustive interleaving proof is claimed.

Deterministic controls cover immediate normal and abnormal child reasons, trapping and nontrapping parents, monitor-only child survival, fresh child trapping state, combined notification order, exact identities/references, capacity/closed/stale/foreign origin rejection, monitor exhaustion and disconnected dispatch. An actual-worker test checks that both relationships exist on its first child poll. Moved input is checked at the original allocation receipt, including rejection. Final review strengthened operand evaluation with three ordered input messages and an order-sensitive child result; constant-valued helpers in the first passing cohort could hide repeated/reordered evaluation. Both cohorts remain retained.

Rust-specific design opportunities are explicit moved input, sealed program and result types, a single publication operation for relationship setup, weak endpoint ownership, and rollback before frame destruction outside locks. Moving input performs no payload copy; the other choices constrain ownership and publication by construction. These are not speed evidence. Ordered tables, locks, frame allocation, command dispatch and contention remain unqualified for performance.

This slice does not add distributed or asynchronous spawn requests, arbitrary spawn options, priority signals, registered names, aliases, general exceptions, arbitrary exit terms, authored direct exits, multiple workers, a real-time timer driver or a supervisor policy. The broader lifecycle gate remains partial; this is not complete BEAM conformance. The next service-level step remains the room/session fixture over this qualified restricted boundary.
