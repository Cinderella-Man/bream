# Safe forced termination of native Rust processes

Research date: **2026-10-07**. Requirements under examination: ordinary Cargo dependency, standard compiler, native Rust process bodies, involuntary preemption, and safe forced termination in version one. Accepted ownership model: process-owned state and owned messages, with no shared mutable state across processes. Ordinary Cargo may include procedural macros and generated Rust code; it does not mean code generation is forbidden. This is a contract investigation; no cancellation experiment was executed. Sources are current Rust language/library documentation, pinned coroutine documentation, and Linux API documentation. The historical C-unwind RFC is identified separately because portions have evolved.

## Finding: the meaning of “terminated” determines feasibility

A runtime can stop scheduling a native execution context. That alone does not establish that it can reclaim its stack, release every resource, or keep shared runtime state usable. **Linux x86_64 plus `FnOnce + Send + 'static` plus runtime-aware I/O and synchronization is insufficient to justify a safe, reclaiming `kill` for arbitrary ordinary Rust code.** This is a concrete gap in the proposed contract, not a claim that all native cancellation is impossible.

| Observable result | Meaning | Assessment |
|---|---|---|
| Stopped, quarantined | Never resume; retain live stack, referenced memory, and necessary thread state | Possible research direction; potentially permanent leaks and locks; not cleanup |
| Deferred stop request | Unwind/return at existing runtime API boundaries | Normal Rust cleanup; an untransformed noncooperating loop may never reach one |
| Terminated and reclaimed | Execution stopped, owned resources released, stack reusable, application remains usable | Not justified for arbitrary native closures by the proposed library-only restrictions |
| Whole OS process exited | Kernel reclaims its private address-space resources | A different isolation boundary; does not itself undo external effects |

**There is a plausible qualified route within ordinary Cargo:** automatically generate native async state machines with mandatory checkpoints, then cancel them through ordinary Rust future destruction. This avoids arbitrary-PC unwinding. It is a restricted supported execution model, not a transparent wrapper around all Rust code; a companion probe tests a small subset, not the full product. Do not relabel quarantine as reclaiming termination.

## A viable native route: generated checkpoints and normal Future cancellation

A procedural macro or other Cargo build-time code generator can transform supported process code into an `async` state machine and insert budget checks at loop backedges and other chosen boundaries. The application author writes no yielding calls. Standard `rustc` generates native machine code and owns the suspended-state representation. On budget expiration, a checkpoint returns `Pending`; on a kill request, the executor stops polling and drops the suspended future normally. **Cancellation must happen after `poll` has returned, never by dropping a future concurrently with an active poll.** [Rust Future contract](https://doc.rust-lang.org/std/future/trait.Future.html), [Tokio's existing suspended-task cancellation semantics](https://docs.rs/tokio/1.53.2/tokio/task/index.html#cancellation)

This is preemption from the process author's perspective through compiler-generated cooperative boundaries, comparable in principle to mandatory reductions/checkpoints. A generated `loop { arithmetic(); }` can be forced to yield despite having no hand-written runtime calls. Memory-safe destruction then uses the language's existing async/pinning machinery, rather than pretending a signal-captured native stack is safely droppable.

The qualified product contract would require:

- Transform every potentially unbounded control-flow cycle in supported process code, including nested functions/closures whose execution is admitted.
- Treat calls into dependencies as bounded trusted operations, transformed process functions, or supported async operations with bounded polls. Reject or explicitly exclude unknown arbitrary callbacks.
- Cover operators, trait dispatch, iterators, formatting, allocators, and macro expansions. For example, rewriting `for` inserts a backedge checkpoint but does not bound a user-defined `Iterator::next`; rewriting arithmetic does not bound an overloaded operator.
- Bound uninterrupted primitives and allocations sufficiently for the promised response target. Source-level checkpoints do not turn a large native operation into constant-time work.
- Enforce owned-message boundaries through constrained payloads/capabilities. `Send + 'static` still admits `Arc<Mutex<_>>`, and safe source code can access mutable globals or spawn native threads.
- Define a separate destructor contract: ordinary drop is memory-safe but need not finish, preserve host availability, or roll back effects.

A simple syntax macro cannot certify those transitive properties. A supported grammar plus sealed operations, audited dependencies, compiler-enforced type restrictions, and explicit rejection of unsupported constructs is a credible first product. This is a **qualified yes**, not an impossible triangle. The outstanding distinction is between “stops executing process code at the next enforced checkpoint” and “all cleanup finishes within a bound even for arbitrary user destructors.”

## Discarding a live Rust stack is not ordinary leaking

The current Rust Reference identifies deallocating a Rust frame without running destructors for its owned locals as a violation of runtime assumptions. A signal handler that jumps to a scheduler and frees/reuses the abandoned stack does not acquire an exemption by being written in Rust. [Rust Reference: runtime assumptions](https://doc.rust-lang.org/reference/behavior-considered-undefined.html#undefinedruntime)

`mem::forget` is safe because Rust does not guarantee every destructor runs. But forgetting a value does not promise that arbitrary pointers remain valid, nor does it authorize invalidating storage that other code still relies on. The distinction is retention versus reclamation. [mem::forget](https://doc.rust-lang.org/std/mem/fn.forget.html)

`Pin` makes this especially concrete: pinned storage must remain valid until the appropriate destruction occurs. Its documentation uses intrusive lists whose neighbors retain pointers to an element. Permanently retaining the element's storage can preserve the invariant; reclaiming the storage without unlinking it cannot. That remains true when the pinned value lives on a green-process stack. [Pin's Drop guarantee](https://doc.rust-lang.org/std/pin/index.html#subtle-details-and-the-drop-guarantee)

Therefore quarantine must preserve more than stack bytes: all backing allocations, registrations, borrowed buffers, and relevant thread state that can still be referenced must remain valid. Keeping a stack mapping but freeing its “process heap” is not automatically safe.

## `'static` at spawn does not prevent later borrowing or escape

This illustrative process has no borrowed captures and can satisfy a `'static` spawn bound:

```rust
runtime.spawn(|| {
    let bytes = [7_u8; 4096];
    std::thread::scope(|scope| {
        scope.spawn(|| loop {
            std::hint::black_box(&bytes[..]);
        });
        loop { std::hint::spin_loop(); }
    });
});
```

The scoped child borrows the parent's local storage. Ordinary Rust is sound because `thread::scope` does not finish before its children join. Reclaiming the parent's green stack bypasses that guarantee and leaves the child referencing invalid storage. Retaining the parent avoids that reclamation bug, but the child continues consuming CPU; it is not automatically part of the runtime's process tree. [Scoped-thread contract](https://doc.rust-lang.org/std/thread/fn.scope.html)

A zero-capture closure can also access static state, create an `Arc`, register callbacks, allocate pinned objects, or invoke a dependency with hidden native threads. Entry ownership is not transitive process isolation. Rust's historical removed scoped-thread API is an instructive precedent: relying on a guard destructor to join was unsound because callers could forget the guard. [Rustonomicon: scoped-thread leak example](https://doc.rust-lang.org/nomicon/leaking.html#threadscopedjoinguard)

## Injecting a panic at an arbitrary instruction is not ordinary unwinding

Rust unsafe implementations may temporarily hold a state that is unsuitable for unwinding, provided no panicking operation occurs during that interval. For example, a container can move an element and repair its bookkeeping using nonpanicking pointer operations before calling user code again. Injecting an asynchronous unwind in the middle invalidates the proof under which the safe container API was implemented. [Rustonomicon: exception safety](https://doc.rust-lang.org/nomicon/exception-safety.html)

There are also explicit boundaries: `GlobalAlloc` requires that implementations not unwind. A timer-triggered panic inside allocation does not become allowed because the process used only safe `Vec`/`Box` APIs. [GlobalAlloc safety requirements](https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html#safety)

Adding `UnwindSafe` to the entry closure does not repair this. The trait is advisory, concerns observing logical invariants across ordinary panic catching, and is not an unsafe trait specifying asynchronous cancellation safety. [UnwindSafe](https://doc.rust-lang.org/std/panic/trait.UnwindSafe.html)

The C-unwind RFC distinguishes ordinary foreign exceptions from forced frame removal and did not define arbitrary asynchronous interruption as supported Rust unwinding. Its older details about catching foreign exceptions must not override current documentation; the useful conclusion is narrower: choosing an unwind-capable ABI is not a general asynchronous-termination contract. [Historical RFC 2945](https://rust-lang.github.io/rfcs/2945-c-unwind-abi.html#forced-unwinding)

## Known suspension points are materially different

Corosensei can force-unwind from its own `Yielder::suspend` boundary with unwind support enabled. Its unsafe force-reset operation explicitly requires that live stack objects need no `Drop`. This demonstrates controlled coroutine cleanup, not safe unwinding from an arbitrary program counter sampled by a signal. [corosensei 0.3.4](https://docs.rs/corosensei/0.3.4/corosensei/struct.Coroutine.html#method.force_unwind)

A library can request cancellation, resume a suspended process through a normal Rust cancellation path, and unwind supported frames. But if the process was interrupted in an endless compute loop, reaching an I/O/receive/sleep boundary is not guaranteed. Scheduling preemption alone supplies no such cleanup boundary. Restoring execution until one happens is deferred cancellation, regardless of whether the scheduler calls the operation `kill`.

Even successful ordinary unwinding has no general bounded cleanup guarantee:

```rust
struct NeverFinishes;
impl Drop for NeverFinishes {
    fn drop(&mut self) {
        loop { std::hint::spin_loop(); }
    }
}
```

A destructor can block, compute indefinitely, or panic. Panicking while already unwinding may abort the host process. Running arbitrary destructors therefore conflicts with unconditional bounded termination and host survival. [Drop documentation](https://doc.rust-lang.org/std/ops/trait.Drop.html#tymethod.drop)

A process can also catch an injected Rust panic with `catch_unwind` and continue into another loop. A private panic payload is not an uncatchable exception. Catching foreign unwinds has implementation-dependent outcomes documented by Rust, including process abort; it is not an escape hatch for reliable isolated killing. [catch_unwind](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html)

## Locks, allocator state, and TLS survive the scheduling decision

Suppose a stopped process holds `Arc<Mutex<State>>`. Retaining the guard can leave every future user waiting forever. Unlocking it externally is not a general solution: the interrupted code may be between invariant-restoring operations, and the guard may have platform thread-affinity requirements. Memory safety, logical consistency, and continued service are distinct promises. [MutexGuard contract](https://doc.rust-lang.org/std/sync/struct.MutexGuard.html)

The same problem extends below user locks to allocators and runtime internals. Linux documents asynchronous pthread cancellation as unsuitable for ordinary allocation and locking because resource state may be inconsistent at interruption; only a very small set of functions is guaranteed async-cancel-safe. `pthread_cancel` does not provide a generic solution for Rust execution. [pthread cancellation safety](https://man7.org/linux/man-pages/man3/pthread_setcancelstate.3.html#NOTES)

Keeping a stopped process on a permanently parked kernel thread avoids reusing that thread's partially updated TLS for another green process. It can still retain global locks and external work forever. Migrating/reusing the worker or freeing TLS needs its own proof; pinning alone establishes neither cleanup nor isolation.

## What restrictions would actually be sufficient to investigate?

For **reclaiming forced termination**, all of these obligations must be enforced transitively, not merely requested in a README:

| Obligation | Why ordinary Rust bounds do not establish it |
|---|---|
| Every possible termination boundary supports valid cleanup or a specified discard mechanism | Standard `rustc` does not expose arbitrary-PC cancellation as a closure capability |
| No borrowed storage escapes the reclaimed domain | `'static` captures do not exclude internal scoped threads, pinning, or registration |
| No partially mutated shared state survives domain destruction | `Send`/`Sync` allow shared globals and shared mutable ownership |
| All native allocations/handles and outstanding operations have an accounted lifetime | Runtime I/O wrappers do not cover arbitrary dependencies or standard-library calls |
| Cleanup cannot run arbitrary blocking/panicking user code | `Drop` imposes no such bound |
| Cancellation cannot be caught and indefinitely suppressed | Ordinary Rust panics are catchable |
| Allocator/TLS/critical sections are either completed or isolated from survivors | A timer signal and private stack do not establish this |

A sealed, runtime-owned program representation with no arbitrary callbacks could enforce many restrictions; a checked source subset compiled by standard Rust could potentially enforce others. Both change the accepted programming model. A procedural macro accepting unrestricted Rust expressions and dependencies does not enforce them automatically. An `unsafe ProcessSafe` trait can place the proof obligation on an audited implementer, but then arbitrary user process registration is not an entirely safe API.

A runtime-controlled heap and mailbox-only sharing are promising architecture tools, **not a sufficient proof by themselves**. In particular, forbidding custom `Drop` still leaves library destructors, pinned state, scoped borrowing, and language-level frame-removal assumptions. No source examined establishes a ready-made safe trait bound for reclaimable arbitrary native stacks.

## Recommended next decision

Keep the standard-compiler native runtime investigation, but specify termination as one of these contracts before accepting an architecture:

1. **Generated safe points:** adopt the supported native Rust subset, insert mandatory checkpoints automatically, and cancel by dropping a suspended future. State cleanup guarantees separately. This satisfies ordinary Cargo and no manual yield calls.
2. **Quarantine:** stop target execution and revoke supported runtime capabilities; retain whatever memory/thread state cannot safely be reclaimed. Expose retained-resource metrics and exhaustion limits. This remains a design to prove, not an automatic guarantee of signal switching.
3. **Reclaim with other restrictions:** accept a checked/audited execution subset and controlled cleanup rules sufficient for reclamation. Define exactly which Rust/dependency patterns are excluded and how that exclusion is enforced.
4. **Reclaim unrestricted native code:** require a stronger isolation boundary or language/runtime support. The ordinary-crate approach currently lacks a demonstrated proof for this promise.

**Subsequent user decisions:** termination must provide BEAM-style runtime-managed cleanup without promising arbitrary user destructor execution. The user also accepted investigating a Cargo-provided process attribute that generates mandatory checkpoints and restricts callable operations/types. Therefore the next proof must combine generated suspension with a controlled cleanup model; dropping arbitrary futures with arbitrary user `Drop` code is not sufficient. No numerical hard deadline was selected.
