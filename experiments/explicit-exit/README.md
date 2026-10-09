# Checked self-exit and targeted local exit

**Thirteenth bounded milestone passed for the local single-worker profile, 2026-10-09.** Checked applications can terminate themselves or signal another local process using five admitted reasons. The public matrix agrees with the pinned BEAM reference. This extends the [previous milestone](../session-replacement/README.md); G1–G5 remain partial. [Milestone tracker](../../docs/milestones.md).

**Correctness only:** no speed tests, throughput/latency measurements or performance comparisons run here. Watchdogs detect hangs. Allowances 1, 4 and 32 exercise checkpoint boundaries; their results do not select a production setting or establish response-time guarantees. [Machine constraint](../../AGENTS.md).

## Public contract

The checked vocabulary adds `self_exit(reason)` and `exit_to(pid, reason)`. Applications do not write awaits. The macro inserts the suspension/control machinery and rejects unsupported operations. The closed reason vocabulary is `Normal=1`, `Killed=2`, `Failure=3`, `NoProcess=4`, `Kill=5`; codes are observation values, not an admitted substitute for typed arguments.

| Operation | Declared behavior |
|---|---|
| `self_exit(reason)` | Models uncaught Erlang `exit/1`. Ends this process with exactly that reason regardless of trapping. No following application statement executes. There is no application catch operation. |
| `exit_to(other, Normal)` | Ignored by a nontrapping target; delivered as a genuine Exit to a trapping target. |
| `exit_to(other, ordinary_abnormal_reason)` | Ends a nontrapping target or delivers Exit to a trapping target. Killed, Failure and NoProcess are ordinary abnormal reasons. |
| `exit_to(target, Kill)` | Untrappable direct kill request; target terminates with Killed. |
| `exit_to(self_id(), Normal)` | Preserves legacy Erlang exit/2 behavior: fatal without trapping, a self-sourced Exit with trapping. |
| `exit_to(dead_or_stale_pid, reason)` | Successful no-op, including when that registry slot has a new occupant. |

A returning targeted call has unit type. It does not report target liveness or acknowledge target death. Fatal self-targets never resume application code. A direct trapped exit between linked peers preserves the link; later linked propagation remains a distinct operation. `self_exit(Kill)` retains Kill, whose propagation through a link is trappable. Direct Kill first becomes Killed, which is also trappable when subsequently propagated. [Pinned source contract](../../docs/research/64-explicit-exit-source-contract.md).

## Implementation and arbitration

The [exit module](runtime/src/exit.rs) shares receipt handling with existing linked exits and uses the lifecycle lock to arbitrate fatal reasons against host kill and terminal commitment. The first accepted fatal event cannot be replaced by a later request. The [runtime integration](runtime/src/lifecycle.rs) validates namespace, slot and generation for both trusted origin and target. An invalid origin is rejected; a missing target is ignored.

Direct local receipt is synchronous under registry/lifecycle arbitration and reads the target's current trapping state there. This **eager local delivery model does not reproduce OTP's queued direct-signal backlog**. Same-sender ordering is checked against already published messages, and converted Exit remains queued when trapping subsequently changes. Delayed direct receipt is not exposed by this interface.

Self-exit records its reason then returns Pending to the worker through an internal future that cannot resume application code. The worker retires the generated frame outside its poll call, using existing bounded relationship/mailbox cleanup. No arbitrary native-stack destruction or application unwinding is introduced. Checked helper operands may suspend; target and reason are evaluated once, left-to-right, before borrowing the process context.

Review found an inherited flaw: synchronous failure to link to an absent process could overwrite an already accepted fatal reason. It now uses the same guarded local claim as self-exit. Regression controls check both event orders. The lock composition is qualified only for the sole application worker; multiple workers require a new lock-order and ownership audit.

## Executed correctness controls

The [public consumer](consumer/src/exits.rs) runs thirty cases per configuration: uncaught self-exit, legacy self-target exit and peer-target exit, each with five reasons and both trapping settings. Six configurations cover debug/release and three allowances. Each checks the observed terminal/message outcome and whether application execution continued.

Two additional cases per configuration stop a computing target through a checked sender using Failure or Kill, without a further receive in the target. Independent peers complete before and after stop. A final case forces same-slot replacement and sends checked Kill to the old PID; the replacement remains unchanged.

Self cases own a Words buffer and an immutable-byte handle inside the process/helper, plus nineteen queued shared envelopes. Retirement releases the queue in three batches. Twenty-one created shared references must all be released, with one physical backing drop after the surviving host reads its bytes and releases its final handle. Peer-target matrix cases require two references and one final backing drop. Original Words allocation receipts also settle once. These counts track admitted storage, not RSS or every runtime allocation.

Ten new [runtime tests](runtime/src/lifecycle/tests/exits.rs) cover exact Down reference/PID/reason, linked propagation, same-sender order, trapping at receipt, stale/foreign identities, first-fatal arbitration and sixteen host-kill/direct-exit races per compiler profile. The full runtime suite now has **105 tests per profile**. The compiler controls reject **159 programs per profile** and execute **nine positive shadowing examples**, including suspended exit operands, exhaustive reason matching and terminal self-exit.

The [BEAM fixture](exit_reference.erl) executes thirty matching matrix cases plus four link-chain, two catch-distinction, one dead-target and two ordering cases. Both the pinned JIT and debug emulator produce the expected thirty-nine rows; all six Rust matrices match both. Catch observations describe BEAM and do not add catch support to Bream.

## Evidence and remaining scope

The [passing cohort](../../evidence/explicit-exit/runs/20261009-exit-01/results.json) records **424 expected command outcomes, 382 aggregate observations and 205 retained inputs**. It also reruns every inherited public control family. The [read-only audit](../../evidence/explicit-exit/validation.json) verifies current/frozen inputs, raw outputs, semantic oracles, OTP build provenance and all 187 previous-milestone inputs. [Evidence guide](../../evidence/explicit-exit/README.md).

Reusing a closed enum, existing terminal arbitration and generated-frame cleanup avoids adding an application unwind path or unbounded user exit callbacks. These are structural design choices, not measured performance improvements.

The profile does not qualify unrestricted Rust, arbitrary exception/reason terms, application catch, ports/aliases, priority signals, distribution, multiple workers or full BEAM exit semantics. Next: one bounded external read operation whose request and buffer remain owned by the runtime independently of the actor, with stop/completion races, stale-result rejection and separately observable cleanup. This applies the [historical green-runtime findings](../../docs/research/63-rust-green-runtime-lessons-and-experiments.md).
