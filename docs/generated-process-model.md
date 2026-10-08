# Generated native processes: the next credible boundary

**Status: research design.** The user accepted investigating a Cargo-provided process attribute, after requiring native preemption, safe forced termination, process-owned state/messages, and runtime-managed cleanup. This is permission to investigate the model, not approval of a production API or a claim that unrestricted Rust functions can be safely killed.

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

Sealed types and generated wrappers can help, but macro expansion in a downstream crate cannot simply access a library's private constructor. A design might use an explicitly unsafe, documented constructor that the verified macro invokes, with safe callers unable to create an arbitrary program through safe APIs. Hygiene, name shadowing, qualified primitive operations and expansion escape paths belong in that proof. No such general public interface is finalized here.

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
