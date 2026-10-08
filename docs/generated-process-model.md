# Generated native processes: the next credible boundary

**Status: research design.** The user accepted investigating a Cargo-provided process attribute, after requiring native preemption, safe forced termination, process-owned state/messages, and runtime-managed cleanup. This is permission to investigate the model, not approval of a production API or a claim that unrestricted Rust functions can be safely killed.

**Implementation update, 2026-10-08:** the [useful-buffer milestone](../experiments/process-milestone/README.md) exercises a restricted public boundary from a separate consumer, using opaque Program construction, trusted concrete input and useful data operations. The [second milestone](../experiments/structured-process/README.md) adds private owning records, scalar tags and acyclic owning helpers that share the caller's allowance. Partial-construction and helper cancellation now have executable controls. Earlier implementation sources and correctness evidence remain; speed-test evidence was removed at user request. General helper/type support and broader resource contracts remain open; this document's wider requirements are not all satisfied.

The [identity/lifecycle milestone](../experiments/identity-lifecycle/README.md) reused the structured compiler apart from its runtime crate binding. It validates namespace/slot/generation, arbitrates accepted kill with completion, separates live lookup from cleanup, publishes prepared frames and settles accepted work on shutdown. The fourth [mailbox milestone](../experiments/mailbox-process/README.md) adds checked `Pid` and `Message` values, consuming send, fixed-tag/any receive and message-field operations. Generated receive suspends automatically; parked tasks wake on send or kill. Queue and frame cleanup settle ownership separately from stopped execution. This passes the fourth restricted correctness profile; richer protocols and runtime notifications remain open, while the following milestone adds injected-clock receive timeouts. The persistent [machine constraint](../AGENTS.md) prohibits speed tests and performance comparisons here.

The fifth [receive-timeout milestone](../experiments/receive-timeout/README.md) adds a concrete owning `ReceiveResult`, checked exhaustive patterns, and generated timed receive. The host explicitly advances logical time; actual timer driving remains unimplemented. A format-equivalence control exposed a missing checkpoint in bare nested match arms; recursive checked lowering now preserves that charge. The retained negative/positive compiler controls and ownership tests remain narrower than general Rust admission.

The sixth [monitor/Down milestone](../experiments/monitor-process/README.md) adds sealed observation references, selectable runtime notifications, cancellation and schedulable flush. Owning events can cross checked helper suspension; application construction of Down and arbitrary selection predicates are rejected. The same mailbox preserves cross-kind arrival order. Weak registration ownership, removable pending work and coalesced wakes avoid retention cycles and redundant service commands without any speed claim.

The seventh [links/exit-trapping milestone](../experiments/link-process/README.md) adds sealed Exit events, fixed-identity selection, trap changes, unlink and explicit typed recovery for an absent local link through `LinkResult`. Fatal linked exit stops execution between polls without an application receive. A weak shared edge represents each pair; links and monitors share an eight-item retirement allowance, with Exit conversion before Down for the same target/observer. Sixty-eight unit tests per profile and checked consumer controls qualify this restricted extension. The following slice adds atomic monitored/linked spawn; direct exit APIs, arbitrary reasons and broad public safety qualification remain open. No speed measurements were made.

The eighth [atomic-spawn milestone](../experiments/spawn-process/README.md) generates sealed factories for statically selected checked child functions. The child receives its own context and checkpoints; its input moves with the existing allocation receipt. `Spawned` exposes only the returned identity and monitor reference. The combined helper/spawn graph retains the prototype's acyclic depth limit. Publication installs the initial link and/or monitor before dispatch, and rejected publication removes topology before dropping the frame outside locks. Eighty-three unit tests per profile and 311 expected command outcomes qualify the restricted boundary, with the following room/session fixture integrating it. This does not admit arbitrary child closures or recover every allocation failure.

The ninth [room/session milestone](../experiments/room-process/README.md) uses those primitives without changing runtime or compiler semantics. Checked private records hold two memberships; one-way monitors isolate session failure, and owned request buffers travel through room, session and collector. Six configurations qualify the bounded conversation, timeout preservation, logical progress beside a CPU loop and teardown. Observed collector count/digest checkpoints are retained separately from the expected transcript. This begins G5 integration but does not qualify dynamic membership, shared immutable fanout or a hogging session.


The tenth [immutable-sharing milestone](../experiments/shared-process/README.md) admits sealed `SharedBytes` and owning shared envelopes, explicit reference cloning, immutable reads and shared selectors in the existing event queue. A private `Arc` owns the transferred vector descriptor and preserves the original backing bytes; every send consumes a handle. Generated helpers can suspend while holding either published shares or an unpublished cloned send operand, and cancellation releases each obligation. Known-handle checksum controls verify surviving reads after publisher and peer death. Physical backing capacity and reference counts have separate receipts; process cleanup may finish while another holder retains bytes. Ninety-five unit tests per profile and the 375-command cohort qualify this restricted extension. Trusted-host ingestion only, fixed membership and allocator-dependent final deallocation remain limits; no speed or latency claim follows.

The eleventh [busy-member milestone](../experiments/busy-session/README.md) composes those contracts without changing runtime/compiler behavior. A monitored room member owns shared bytes and scratch state in a generated infinite loop. Acyclic helpers move that owning state across generated checkpoints; healthy-peer requests complete while the busy member continues work. Direct kill and fatal linked exit stop it with no further receive; exact Down, post-stop reads, frozen application observations and batched queued cleanup are checked separately. Six configurations cover eighteen scenarios and pass the 381-command cohort. Shared reference creation stays constant during helper computation and replies, demonstrating an ownership choice without making a speed claim. Fixed membership, one worker and the restricted grammar remain explicit limits.

The twelfth [session-replacement milestone](../experiments/session-replacement/README.md) also composes inherited runtime/compiler contracts. A private slot record is replaced with a fresh sealed PID/monitor pair while the retired receipt remains distinct. A three-slot registry forces reuse of the old member's vacated slot; selective receive preserves its old reply and genuine Down until after new-member readiness. Epochs distinguish trusted application requests only, without authenticating senders. Old-PID sends release their transferred ownership, and both new and surviving members retain readable shared bytes. Six configurations cover twenty-four scenarios, passing 387 expected command outcomes. This adds neither general dynamic membership nor a restart framework; direct checked exit APIs remain the next bounded extension.

## Why generation matters

An opaque native closure can allocate, enter another library, create scoped threads borrowing its stack, or run arbitrary destructors. Signals can suspend its instructions, but do not tell the runtime how to reclaim its state safely. The [native signal probe](../experiments/native-preemption/README.md) answers scheduling; the [termination review](research/05-forced-termination.md) explains the gap.

Generated process code gives the runtime a different boundary. The attribute transforms supported application control flow into a native Rust state machine with mandatory budget checkpoints. Standard rustc owns the suspended representation. When a checkpoint returns control, the executor may run a peer, resume the process, or destroy its supported state through ordinary Rust cleanup. The author writes no yields, and a tight supported loop cannot omit its turn boundary.

The execution may use futures internally, but this does not inherit ordinary Tokio's inability to interrupt a non-returning application poll: all admitted process loops must be transformed so their polls return. It also does not make *arbitrary* futures preemptible. A future created outside this checked boundary is not a process program.

Use precise terminology when presenting this publicly: the signal probe demonstrates asynchronous native interruption; this model implements the author-facing preemption promise through **mandatory generated checkpoints**. Internally that is cooperative suspension of generated state machines, but application code cannot choose to omit it within the admitted grammar. It is neither arbitrary-instruction interruption nor an API asking authors to insert `yield_now`.

## A small enforceable experiment

The [generated-process probe](../experiments/generated-process/README.md) executes a deliberately closed grammar: primitive integer state, loop/control-flow forms, and a few runtime-owned operations. It demonstrated two native processes progressing on one worker, termination of an infinite process after a generated checkpoint, reclamation of its tracked 4,096-byte buffer, and continued progress by the survivor. Ten unsupported-source fixtures fail compilation through the checker. A separate empty infinite loop proves that loop-body runtime calls are not needed for suspension.

That experiment establishes a mechanism for the admitted subset. It does not establish that a general Rust procedural macro can validate every dependency or that the subset is yet useful for stateful services.

## Rules the real model would need

| Boundary | Required enforcement |
|---|---|
| Program construction | Safe spawning accepts only a validated/generated program. A public constructor taking arbitrary `Future` or `FnOnce` would bypass the model. |
| Loop control | Every admitted cycle reaches a mandatory checkpoint. `continue`, nested loops, recursion and macro expansion must not bypass it. |
| Called code | Calls are transformed process calls or approved native/runtime primitives. Arbitrary synchronous callbacks are not silently accepted. |
| Type behavior | Primitive-looking operators cannot dispatch to unknown user code; iterators, conversions, indexing and formatting need explicit treatment. |
| State and messages | Only approved owning data shapes/capabilities; no shared mutable graph or escaping borrow into another lifetime. |
| Cleanup | Runtime-owned types have controlled cleanup. Custom user destructors, hidden registered callbacks, native child threads and other untracked lifetimes require rejection or a separate audited contract. |
| Poll ownership | Exactly one worker polls a process; stop/free occurs only after the active poll has returned. |
| Resource ownership | Admission, transfer, execution stop, external-operation completion and reclamation have ordered ownership transitions. |
| Host integration | Approved allocation/I/O operations have documented suspension and cleanup behavior; their own native work is part of the compatibility/latency model. |

Procedural macros operate on syntax, not a complete compiler-certified effect system. A broad Rust body with an added `.await` at every visible loop is not enough: `some_trait_method()`, overloaded `+`, `Iterator::next`, callbacks, or a dependency can hide a non-returning computation. Even a closure with no captures can access globals. A restricted grammar can reject these; a general Rust surface needs a stronger transitive mechanism.

Sealed types and generated wrappers can help, but macro expansion in a downstream crate cannot simply access a library's private constructor. The new restricted milestone uses an explicitly unsafe, documented factory constructor invoked by the checking macro; ordinary safe callers cannot use that escape hatch. Explicit crate bindings and consumer shadowing controls protect the generated paths. Hygiene, primitive operations and expansion escapes remain part of the argument as the grammar grows. No general public interface is finalized here.

## Kill with runtime-managed cleanup

The intended observable sequence is:

```text
kill requested
  → active generated poll reaches its mandatory boundary
  → program is marked non-runnable and cannot resume
  → ordinary cleanup releases the approved state and runtime registrations
  → owned asynchronous work is cancelled/joined/reconciled under its contract
  → termination completion reports what has actually been reclaimed
```

This is forced with respect to the process author: the loop cannot suppress a generated boundary or catch cancellation as an application panic. It is not arbitrary-instruction destruction, and its latency includes the remaining work until the next boundary plus approved native operations and cleanup. A hard time bound is not implied.

The selected contract omits arbitrary user destructors, not Rust's memory-safety obligations. Generated frames still require valid ordinary destruction. A plausible design admits data whose cleanup is wholly supplied by the runtime and standard, audited owning primitives. A per-process arena alone does not make arbitrary values safe to discard, nor does it solve pointers that escaped that arena.

Outstanding external effects deserve their own result. Closing a process-owned handle does not roll back a write already accepted by another system. Owned-message transfer likewise must specify when ownership leaves the sender so killing it cannot free a payload already delivered to a receiver.

## What “generic enough” would mean here

The process model should eventually cover typed owned structs/enums, persistent state, process calls, owned messages, timers and asynchronous I/O, plus useful native computation. Actor behavior traits and supervision are optional higher layers. The test of generality is two independent consumers sharing the runtime without carrying the original lab's domains or protocols.

There is a real product risk: a checked subset can become so restrictive that it feels like a new embedded language rather than ordinary Rust. The next design review should use concrete example programs—an in-memory stateful service and a CPU-heavy computation with helper calls—to judge that cost. Do not conceal this tradeoff by calling the attribute “just a macro”.
