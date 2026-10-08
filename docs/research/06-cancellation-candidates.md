# Native cancellation candidates: stopping is not safe reclamation

Research date: 2026-10-07. Scope: source inspection, not a cancellation experiment or a proof of soundness. This supplements [the runtime landscape](03-runtime-landscape.md) after the requirements became native Rust, ordinary Cargo distribution, automatic preemption, and safe forced termination. The user has accepted process-owned state and owned messages, without mutable state shared between processes. That helps, but an ordinary Rust entry closure does not enforce the same restriction on everything called inside it.

## Conclusion for Bream

The audited native candidates demonstrate useful scheduling mechanisms, but none supplies an established contract for safely killing an arbitrary, unchanged Rust computation at any instruction, reclaiming its state, and keeping the host correct. In particular, open-coroutine's cancellation path cannot be imported as a safe `kill` implementation: it abandons suspended frames using an unsafe reset, then permits the stack mapping to be reclaimed. May's cancellation uses a panic at a cooperative boundary, which an uninterrupted CPU loop never reaches. Corosensei provides proper unwinding from its suspension protocol, not a generally valid arbitrary-instruction cancellation protocol.

This does **not** establish that a custom rustc is logically necessary. A Cargo proc macro or source generator could introduce mandatory, automatic checkpoints or produce a typed state machine for a closed supported code surface. The application would never call `yield`. Safe cancellation could then occur at compiler-visible boundaries and use ordinary Rust destruction. This is a viable research direction if the project accepts qualified code and dependencies, rather than accepting arbitrary `FnOnce` bodies and promising transitive instrumentation that a macro cannot perform.

## What the candidate APIs actually do

| Candidate and inspected version | Running-task cancellation | Cleanup and reclamation | Consequence |
|---|---|---|---|
| open-coroutine / core / hook 0.7.0, published 2025-05-28 | Unix request sends SIGVTALRM to the scheduler thread; handler suspends the current coroutine with a cancellation marker. | Scheduler drops the cancelled coroutine; wrapper `Drop` calls unsafe `force_reset`, bypassing ordinary corosensei stack unwind. | Mechanically stops some no-yield computation; does not establish safe general-purpose kill. |
| open-coroutine current pinned revision `d725ceab4b780d57c9aa88777a96742e31549914`, dated 2026-10-06 | Same broad signal/marker design; task IDs have changed. | Same reset in wrapper `Drop`. | The published limitation has not disappeared from this audited path. |
| corosensei 0.2.2, compatible with open-coroutine 0.7.0's `corosensei = "0.2"` dependency | `force_unwind` resumes a coroutine suspended through the yielder with a special unwind payload. | Drops stack locals through unwind; `Drop` aborts if the required unwind cannot complete. | Useful at supported suspension boundaries; cannot independently interrupt a running tight loop. |
| corosensei 0.3.4, published 2026-05-24 | Same essential suspension requirement; unwind handling has evolved. | Distinguishes safe `force_unwind` from unsafe `force_reset`. | Upgrade alone does not make arbitrary signal interruption safe to unwind. |
| May 0.3.51, published 2025-05-13 | Unsafe `cancel` sets a flag and may wake a parked/I/O coroutine. A later cancellation check raises a panic. | Panic unwinds when the check is reached, subject to panic mode and destructor behavior. | No no-yield CPU-loop termination guarantee. |

These conclusions follow the sources below, rather than treating the existence of a method named `cancel` as evidence of a process-isolation contract.

### open-coroutine 0.7.0: complete request-to-drop path

1. Public `JoinHandle::try_cancel`, `open-coroutine/src/lib.rs:214–223`, calls the hook ABI. Hook `src/lib.rs:108–115` returns success after dispatching `EventLoops::try_cancel_task`; it does not wait for a terminal outcome or completed cleanup. [Public source](https://docs.rs/crate/open-coroutine/0.7.0/source/src/lib.rs), [hook source](https://docs.rs/crate/open-coroutine-hook/0.7.0/source/src/lib.rs).
2. Core `co_pool/mod.rs:438–474` finds a running task and its scheduler thread, then sends SIGVTALRM on Unix. Otherwise it queues cancellation. Windows running-task support is explicitly left as a TODO in this code. [Pool source](https://docs.rs/crate/open-coroutine-core/0.7.0/source/src/co_pool/mod.rs).
3. Core `coroutine/mod.rs:140–172` installs an `extern "C"` signal handler. It finds the current suspender and calls `cancel`. `suspender.rs:54–67` sets a thread-local marker and suspends; this is not stack unwinding. [Handler source](https://docs.rs/crate/open-coroutine-core/0.7.0/source/src/coroutine/mod.rs), [suspender source](https://docs.rs/crate/open-coroutine-core/0.7.0/source/src/coroutine/suspender.rs).
4. Core `coroutine/korosensei.rs:438–450` converts the marker into `CoroutineState::Cancelled`. In `scheduler.rs:293–352`, that state is not returned to a runnable queue; the local coroutine is dropped. The queued cancellation branch also drops its coroutine by continuing the loop. The complete/error branches insert results, while the cancelled branch is empty; terminal observation and join semantics deserve a separate audit. [Coroutine source](https://docs.rs/crate/open-coroutine-core/0.7.0/source/src/coroutine/korosensei.rs), [scheduler source](https://docs.rs/crate/open-coroutine-core/0.7.0/source/src/scheduler.rs).
5. The wrapper's `Drop`, `coroutine/korosensei.rs:365–373`, calls `self.inner.force_reset()` whenever execution started but has not completed. In corosensei 0.2.2 this sets the stack pointer to `None`, preventing its normal drop-time unwind. The default Unix stack's destructor then calls `munmap`, `stack/unix.rs:85–95`. Thus this path does not merely quarantine the whole stack forever: it can reclaim the stack while bypassing its local destructors. [Reset implementation](https://docs.rs/crate/corosensei/0.2.2/source/src/coroutine.rs), [stack allocation destructor](https://docs.rs/crate/corosensei/0.2.2/source/src/stack/unix.rs).

Current HEAD preserves the wrapper reset at lines 380–388 and the running cancellation routing at lines 457 onward. These links are pinned: [current wrapper](https://github.com/acl-dev/open-coroutine/blob/d725ceab4b780d57c9aa88777a96742e31549914/core/src/coroutine/korosensei.rs#L380), [current pool](https://github.com/acl-dev/open-coroutine/blob/d725ceab4b780d57c9aa88777a96742e31549914/core/src/co_pool/mod.rs#L457), [current handler](https://github.com/acl-dev/open-coroutine/blob/d725ceab4b780d57c9aa88777a96742e31549914/core/src/coroutine/mod.rs#L147).

The signal targets a thread, while the handler acts on its current suspender. Correct task identity across lookup, dispatch, delivery, and task completion is another required audit. This inspection did not reproduce a wrong-task kill and does not claim that one has been proved. Similarly, signal-handler reentrancy, held allocator/stdlib locks, and native-call interruption require separate evidence; the existence of a signal switch is insufficient. [Linux signal-safety requirements](https://man7.org/linux/man-pages/man7/signal-safety.7.html).

### Corosensei: the unsafe precondition matters

Both inspected versions explicitly distinguish `force_reset` and `force_unwind`. Reset has a longjmp-like safety condition: the suspended stack must have no objects requiring destruction. Force-unwind instead drives the coroutine's supported unwind path; unwinding a started suspended coroutine requires the unwind feature and `panic=unwind`. Failure during drop can abort the whole host. [0.2.2 implementation, lines 304–401 and 470 onward](https://docs.rs/crate/corosensei/0.2.2/source/src/coroutine.rs), [0.3.4 API](https://docs.rs/corosensei/0.3.4/corosensei/struct.Coroutine.html#method.force_unwind).

Calling an unsafe function is not automatically undefined behavior; satisfying its preconditions can make it correct. Conversely, surrounding the call with `unsafe` does not establish those preconditions for arbitrary user code. Open-coroutine's constructor accepts ordinary closure execution, without a visible type-level proof that suspended frames are reset-safe.

A signal-handler suspension technically passes through the yielder, but that alone does not qualify the interrupted application frames for asynchronous unwind. Blindly replacing reset with `force_unwind` would introduce the signal handler's non-unwind C ABI and arbitrary interrupted Rust/foreign frames into the proof obligation. A Rust panic crossing a non-unwind ABI boundary aborts; unwind metadata by itself does not establish that every interrupted operation has valid cleanup semantics. [Rust FFI and unwinding](https://doc.rust-lang.org/nomicon/ffi.html#ffi-and-unwinding).

Corosensei also exposes trap recovery, but its documented safety contract sharply restricts interaction with memory outside the coroutine and requires yielding for environment operations. It is evidence that controlled execution can discard state under a restrictive contract, not a general native-Rust cancellation solution. [0.2.2 trap contract, `setup_trap_handler`](https://docs.rs/crate/corosensei/0.2.2/source/src/trap.rs).

### May: cancellation is observed at scheduling boundaries

May's public `Coroutine::cancel` is unsafe. Its implementation sets the cancellation flag and wakes eligible parked/I/O work. `check_cancel` eventually raises `generator::Error::Cancel` through `panic_any`, except while already panicking. The public documentation ties termination to a later scheduling boundary. A CPU loop containing no runtime call never performs this check. [Published-version public API, lines 138–146](https://github.com/Xudong-Huang/may/blob/85fa1ee2bc7e5bd8d0c40a207de542cc46e501f7/src/coroutine_impl.rs#L138), [cancellation implementation, especially lines 111–140](https://github.com/Xudong-Huang/may/blob/85fa1ee2bc7e5bd8d0c40a207de542cc46e501f7/src/cancel.rs#L111).

POSIX asynchronous thread cancellation does not provide an unrestricted escape hatch either. Its documented suitability excludes ordinary resource-owning/allocating code; the async-cancel-safe set is exceptionally small. It is relevant to tightly controlled pure computation, not a proof that arbitrary Rust destructors and native libraries can be interrupted safely. [pthread cancellation documentation](https://man7.org/linux/man-pages/man3/pthread_setcancelstate.3.html).

## Why ownership alone does not permit arbitrary stack disposal

Rust permits resource leaks. Forgetting an owned value and omitting its destructor is not, by itself, undefined behavior. The standard library explicitly allows this. But `mem::forget` also does not promise that pointers remain valid; it is not permission to invalidate a live borrowed stack. [The `forget` safety contract](https://doc.rust-lang.org/std/mem/fn.forget.html).

Separately, the current Rust Reference lists deallocation of active Rust frames without running their owned local destructors as a violation of runtime assumptions, and identifies longjmp as a possible cause. Safe, explicit forgetting of a value and asynchronous abandonment of its owning frame are different operations. [Rust Reference, runtime assumptions](https://doc.rust-lang.org/reference/behavior-considered-undefined.html#undefined.runtime).

A concrete counterexample to an unrestricted `'static` closure boundary is an entry function that creates local data, starts a scoped OS thread borrowing that data, and then computes indefinitely. No borrow had to enter through the closure arguments. Normal `thread::scope` guarantees joining the child before leaving the scope; discarding the parent stack bypasses that guarantee. The child could continue accessing unmapped memory. This is a reasoning counterexample, not an unsafe test run. [Scoped-thread guarantee](https://doc.rust-lang.org/std/thread/fn.scope.html).

Likewise, callees can use global locks, TLS, callbacks, or native operations despite receiving owned inputs. For process-owned state to be an enforced contract, the allowed transitive operations must be constrained too. Cross-process owned messaging alone is insufficient.

Leaking the entire suspended stack and its captured ownership indefinitely may avoid the immediate unmapped-stack failure under additional lifetime/invariant conditions. It is a different proposed design, not the audited open-coroutine behavior. It does not release locks, repair interrupted library invariants, finish pending external operations, or bound retained memory across repeated kills. It cannot be reported as successful reclamation or BEAM-like recovery without those qualifications.

## A Cargo-only route worth testing

A supported macro-generated computation can contain automatic loop/backedge checkpoints and calls only to other generated computations or explicitly qualified bounded operations. Cancellation is forced by the runtime at the next mandatory generated boundary: user code cannot omit a manual yield because none is required. A generated state machine can be dropped when it is no longer being polled; a stackful backend can unwind from a deliberate, qualified suspension point. These are design proposals, not capabilities established in the audited libraries.

The boundary must cover recursion, loops introduced by expansion, dynamic calls, iterators, dependency calls, native calls, and cleanup. Injecting checks around a call to an uninstrumented function that never returns does not make that function killable. A proc macro cannot inspect and rewrite the entire transitive program, so unsupported operations must be rejected, explicitly outside the guarantee, or moved behind a stronger isolation boundary. Describe this honestly as generated automatic preemption for qualified Rust, not arbitrary-code preemption.

Destructors are a separate liveness problem. An otherwise safe `Drop` can compute indefinitely, block, or panic. If the requirement includes bounded completion of reclamation, supported cleanup must also have a bounded/qualified contract or use runtime-owned resources with controlled teardown. A cancellation acknowledgment should identify whether application execution stopped, cleanup completed, and resources were reclaimed; a successful request dispatch is not all three. External side effects cannot generally be rolled back by task destruction.

## Decisive qualification gates

- Kill an instrumented infinite arithmetic loop without explicit yields; demonstrate peer progress, confirmed terminal state, and non-resumption.
- Repeat kill/recreate cycles with process-owned allocations and runtime resource handles; measure stable retained memory and correct cleanup counts.
- Reject or explicitly disqualify an uninstrumented infinite callee, scoped-thread stack borrow, and unbounded destructor. Merely accepting their enclosing `FnOnce + Send + 'static` is a failed contract test.
- Verify partially initialized state, pinned/self-referential generated state, cancellation racing normal completion, and cancellation during supported I/O. Keep memory-safety tests distinct from fairness tests.
- For an unwind backend, qualify panic strategy, swallowed cancellation panics, nested destructor panics, and foreign ABI crossings. For a state-machine backend, define cleanup panic handling without promising that `panic=abort` preserves the host.
- For signal-assisted designs, test delivery identity and deferred cancellation around every supported critical region. A mechanism smoke test consisting only of arithmetic/atomics cannot establish stdlib, allocator, FFI, TLS, or cleanup safety.

## Evidence preservation and limits

Exact published sources are under `/home/bat/projects/bream/upstream/cancellation-review/` (open-coroutine/core/hook 0.7.0, corosensei 0.2.2/0.3.4, May 0.3.51). Current open-coroutine excerpts are under `/home/bat/projects/bream/upstream/native-review/`. Release source links use versioned docs.rs because the published open-coroutine `.cargo_vcs_info.json` revision `10431e991ab60ea1cd0bcadfe454adca511e268e` did not resolve through raw GitHub during inspection. May's release revision does resolve and is pinned above.

No production code was changed and no potentially unsound forced-cancellation experiment was run for this report. rrkernel's prior scheduling audit is not evidence of a qualified cancellation/reclamation API; its full cancellation semantics were not audited here. The report identifies constraints and disqualifies an unsupported general guarantee, rather than proving the full soundness or unsoundness of any entire crate.
