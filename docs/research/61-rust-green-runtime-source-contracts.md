# Rust's final green runtime: what the code actually guaranteed

Research date: **2026-10-09**. Source audit only: no historical code was executed, no benchmarks or speed comparisons were run for this document. The final-2014 snapshot inspected here is Rust commit **`770378a313a573776b16237a46b75bafa49072c1`**, before runtime-removal PR #18967. Selected original files and their hashes are retained in [the source manifest](../../upstream/rust-green-history/770378a313a573776b16237a46b75bafa49072c1/source-manifest.json). Earlier linked-failure machinery and removal chronology require their own pinned revisions; this document does not project the final snapshot backward onto all pre-1.0 Rust.

## Finding

**Rust's green runtime did not solve arbitrary native preemption or general forced termination and then disappear. It offered stackful cooperative tasks, runtime-aware blocking operations, ordinary panic containment, and explicit compatibility limits.** Its implementation contains useful mechanisms for Bream, and concrete examples of problems Bream must still solve.

The source explicitly identifies communication and runtime I/O boundaries as scheduling opportunities and warns that an infinite loop monopolizes its scheduler. It separately warns about native blocking calls, native thread-local storage, native synchronization and `fork`. These are stated limitations, not deductions from Rust's later move to async. [Original architecture and caveats](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/lib.rs#L49-L110).

This establishes neither that Rust cannot support a stronger runtime nor that every dependency needs a permanent allowlist. It establishes that the existence of historical green threads is not evidence that those stronger contracts had already been implemented.

## Execution, scheduling and stack ownership

`GreenTask` owns a `Coroutine`, comprising its saved execution context and stack. It records its scheduler pool and can be restricted to a particular scheduler. Ordinary tasks can migrate within their pool. Creation allocates a stack and initializes a bootstrap frame; it does not rewrite arbitrary application instructions to make them preemptible. [Task representation and construction](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/task.rs#L37-L160).

`Context::swap` records the incoming Rust stack bounds and calls `rust_swap_registers`. The context documentation distinguishes registers required at a normal context-switch call from a complete arbitrary-instruction register image: caller-saved values needed after the call are already the caller's responsibility. **This context-switch routine is not, by itself, an asynchronous preemption implementation.** [Context switch and register contract](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/context.rs#L82-L146).

The scheduler's `yield_now` switches to the scheduler task and enqueues the yielding task. `maybe_yield` counts calls into that runtime operation before yielding. This is a count of reached cooperative opportunities, not an instruction budget automatically enforced inside an opaque function. [Yield paths](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/sched.rs#L829-L876).

Each stack uses a fixed mapping with a guard page in this snapshot. `Stack` explicitly describes its name as a remnant of segmented stacks; context source says segmented stacks are disabled. The stack pool searches for a sufficiently large cached stack and otherwise allocates a mapping. Completed stacks return to the pool. Neither guard pages nor pooling make suspended user frames disposable. [Stack allocation and pool](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/stack.rs#L17-L181), [segmented-stack qualification](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/context.rs#L142-L146).

**Bream implication:** separate three mechanisms in the design and evidence: continuation storage, permission to switch, and permission to reclaim. A stackful implementation can make suspension transparent through deep ordinary call chains without proving arbitrary interruption or destruction safe. Pooling is a possible representation optimization, not evidence of better performance on this machine.

## An unusually relevant race: waking before registration finishes

The registration callback in `deschedule_running_task_and_then` borrows state on the suspended task's stack but executes from the scheduler stack. It may publish the blocked task before it finishes using that borrowed state. Another scheduler could then resume the original task concurrently. The implementation holds a per-task lock while the callback runs; resumed execution acquires that lock before returning from the context switch. [Callback contract and guard](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/sched.rs#L718-L779), [resumption barrier](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/sched.rs#L662-L675).

The original [issue #8132](https://github.com/rust-lang/rust/issues/8132), opened July 2013, explains this ownership gap and proposes an alternative: a noncopyable environment-permission token consumed when scheduling the task. That would prevent subsequent safe access through the token. It explicitly notes that raw-pointer access still needs auditing.

This yields two independently necessary properties for a Bream wait primitive:

1. One contender wins the right to wake a task.
2. Execution and reclamation wait until every registration callback borrowing that task's state is finished.

An atomic winner flag can establish the first without the second. A notification being published is not evidence that its publisher has released every borrowed reference. The same distinction matters for cancellation racing with completion.

The historical `BlockedTask::Shared` representation uses an atomic exchange to let one claimant recover the task pointer. Its `make_selectable` path creates several handles to the same claim. That mechanism establishes single ownership, while the separate scheduler lock protects the callback's remaining lifetime. [Claim and selection implementation](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/librustrt/task.rs#L467-L507).

**Bream idea:** prefer an API that consumes ownership of registration state before publishing readiness. Where a backend genuinely borrows task storage, model a separate registration-finished phase. Do not reproduce the historical lifetime-erased closure interface merely because its lock made the observed path work.

## Panic, cleanup and final stack reclamation

The green bootstrap executes user code inside `Task::run`, calls `destroy`, and only then enters green-task termination. This is normal return or panic-unwind completion followed by runtime cleanup, not an external instruction that destroys an arbitrary running stack. [Bootstrap](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/task.rs#L84-L126).

`Task::run` catches task failure. `cleanup` removes and destroys task-local storage, invokes the exit callback and changes the task's state to destroyed. User destructors can run in this path. The source records known task-local destruction problems, including leaks and reentrant task-local access. These historical comments should not be treated as present-day Rust bugs, but they disprove a reading in which green tasks had universal, trouble-free cleanup. [Task execution and cleanup](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/librustrt/task.rs#L210-L327).

The unwinder aborts when a task panics while already unwinding. Thus containing an ordinary task panic was not an unconditional guarantee that every user destructor failure stayed within that task. [Double-panic path](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/librustrt/unwind.rs#L579-L602).

After cleanup, `terminate_current_task` switches away from the dying task. A callback on the destination context recycles its stack and decrements the pool's active count. The task does not unmap the stack from which it is still executing. [Final switch and recycling](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/sched.rs#L799-L813).

**Bream idea:** retain distinct acknowledgements for logical stop, user cleanup completion, registration quiescence and backing-storage reclamation. A stopped identity can reject messages before all physical storage is reclaimable. Exit notification should name the phase it certifies; it must not implicitly promise that an opaque destructor or unrelated native thread has finished.

### Do not mistake stale kill terminology for implementation

Some final-snapshot comments still refer to killed tasks and killers. However, the inspected `Death` state stores an exit callback, and the actual `BlockedTask::block` path simply creates an owned blocked task. `Shared` implements an atomic claim useful for selection. These comments alone do not demonstrate a public cancellation mechanism or arbitrary asynchronous kill. [State definitions](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/librustrt/task.rs#L135-L147), [constructors and claims](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/librustrt/task.rs#L467-L547).

Earlier linked-failure code must be traced at its own revision, including where requests are observed and how unwinding begins. A flag named `killed` is insufficient evidence of arbitrary-instruction cancellation.

## Task-local data was explicitly virtualized; arbitrary native TLS was not

Runtime task-local values live in a map owned by `Task`, reached through the currently installed runtime-task pointer. A fresh task starts with no map; the task-local test confirms that a child does not inherit its parent's value. The representation follows task ownership rather than the OS worker's identity. [Task-local representation](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/librustrt/local_data.rs#L74-L151), [child-isolation test](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/librustrt/local_data.rs#L409-L431).

A particularly revealing implementation comment explains why runtime TLS accessors are marked `#[inline(never)]`: optimization could retain an OS TLS base across a green-task migration. An access after migration could then use the original worker's TLS. The runtime forced fresh loads in its own accessors. This is a historical compiler/runtime coordination workaround, not a modern blanket recommendation to add `inline(never)` to arbitrary TLS helpers. [Full rationale and helpers](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/librustrt/local_ptr.rs#L103-L204).

**Bream implication:** saving the stack and ordinary registers does not by itself virtualize worker identity, native TLS, or libraries whose invariants depend on remaining on a particular OS thread. A nonmigrating lane, task-owned context, and qualified foreign-call boundaries are different tools with different contracts. Pinning to one worker addresses migration, but does not make a blocking call on that worker cooperative.

## Native boundaries and snapshot limitations

The runtime interface has separate yield, deschedule, wake, sibling-spawn and `can_block` operations. Green tasks return false for `can_block`; native tasks have a different implementation. This provides a precedent for exposing execution-context capabilities instead of inferring them from `Send`. It does not intercept arbitrary OS calls. [Runtime interface](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/librustrt/lib.rs#L69-L101), [green answer](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/task.rs#L483-L490).

The final tree is already in transition: `rtio.rs` describes `has_active_io` as the remaining piece of `IoFactory`, while standard timer code directly uses the system backend. Do not use the architecture overview's I/O wording to assume every standard I/O operation at this exact revision still cooperatively deschedules. A Rust 0.12/libuv execution probe is a separate baseline. [Remaining event-loop interface](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/librustrt/rtio.rs#L11-L27), [timer backend](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libstd/io/timer.rs#L23-L27).

The pool's shutdown waits for its own tasks, not tasks created in native mode or other pools. That is pool completion, not universal descendant containment. [Shutdown contract](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/lib.rs#L458-L484).

## Bounded experiments suggested by the source

These are proposed correctness experiments, not reported outcomes. Use explicit acknowledgements, controlled release messages, finite arithmetic and exact ownership receipts. Watchdogs detect hangs only.

| Question | Negative case | Control and interpretation |
|---|---|---|
| Does stackful scheduling imply CPU preemption? | On one historical scheduler, perform a finite arithmetic phase with no runtime calls before recording a phase marker. | Add explicit yields. Compare event ordering, not elapsed time. Establish reached scheduling boundaries only. |
| Is winning wake ownership sufficient? | Publish wake while a registration callback still owns a logical permission to access task state; allow a modeled resume immediately. | Require callback-finished acknowledgement or consume an environment token before readiness publication. Check forbidden phase overlap without dereferencing invalid memory. |
| Is stack storage immediately reclaimable after stop? | Model stop acknowledgement while an external completion registration is still active. | Reclaim only after registration release and the final switch away. Record ownership states, not unsafe early frees. |
| Does migration preserve native TLS? | Move a logical task between two controlled workers with different thread-local markers. | Task-owned context stays constant; worker-local marker changes. This demonstrates identity semantics, not the old optimizer bug. |
| Does pinning make blocking cooperative? | Pin one task to a worker and enter a channel-controlled native wait. | A second worker continues; tasks confined to the first progress only after release. Use handshakes and a releaser outside the blocked worker. |
| Does pool completion include foreign children? | Spawn a native child with an explicitly controlled release after its green parent returns. | Record pool-local versus child completion separately; always join the native child. |

The most immediately reusable finding is the **registration-lifetime boundary**, because Bream already models wake, timeout, cancellation and identity reuse. The broadest architectural finding is that synchronous-looking application APIs can coexist with stackful suspension, but compatibility still depends on what the runtime owns and which operations cooperate. This history gives Bream design tools and precise obligations; it does not validate arbitrary-library preemption, guarantee forced kill, or establish any performance ranking.

## Exact Rust 0.12 baseline for executable probes

The official Rust 0.12 compiler reports abbreviated revision `ba4081a5a`. Resolving the official `0.12.0` Git tag gives full commit **`ba4081a5a8573875fed17545846f6f6902c8ba8d`**. This is distinct from the final-2014 removal snapshot above. Eighty selected source/license files have been retained and hash-checked in [its separate manifest](../../evidence/rust-green-history/sources/ba4081a5a8573875fed17545846f6f6902c8ba8d.json); the source archive SHA-256 is `f9fd61ea00214568078c6cb99da527d65ec8e91eecb639b39340c952b5e2e686`. No executable results are asserted in this source audit.

At this exact revision, a one-worker pool is configured with `PoolConfig { threads: 1, event_loop_factory: green::basic::event_loop }`. `TaskBuilder::new().green(&mut pool).try_future(proc() { ... })` returns the historical result future, whose `unwrap()` waits for task completion and returns a success/failure result. `std::task::deschedule()` is an explicit yield. Task-local storage uses `local_data_key!(name: uint)`, or the equivalent static `std::local_data::Key<uint>`; the spelling is not modern `task_local!`. [Pool configuration](https://github.com/rust-lang/rust/blob/ba4081a5a8573875fed17545846f6f6902c8ba8d/src/libgreen/lib.rs#L287-L305), [task result and yield APIs](https://github.com/rust-lang/rust/blob/ba4081a5a8573875fed17545846f6f6902c8ba8d/src/libstd/task.rs#L283-L318), [yield](https://github.com/rust-lang/rust/blob/ba4081a5a8573875fed17545846f6f6902c8ba8d/src/libstd/task.rs#L380-L388), [task-local macro](https://github.com/rust-lang/rust/blob/ba4081a5a8573875fed17545846f6f6902c8ba8d/src/libstd/macros.rs#L294-L315).

**Probe confound:** both channel send and nonblocking `try_recv` periodically invoke `maybe_yield`. A loop polling a channel is therefore not a valid no-scheduling-opportunity negative control. Keep an opaque finite computation free of channel calls and other runtime services; use atomics for internal phase markers if needed. The 0.12 timer backend still uses `LocalIo`/`IoFactory`, unlike the final snapshot, and the basic green event loop does not supply I/O. [Channel send scheduling](https://github.com/rust-lang/rust/blob/ba4081a5a8573875fed17545846f6f6902c8ba8d/src/libsync/comm/mod.rs#L608-L622), [try-receive scheduling](https://github.com/rust-lang/rust/blob/ba4081a5a8573875fed17545846f6f6902c8ba8d/src/libsync/comm/mod.rs#L852-L862), [timer construction](https://github.com/rust-lang/rust/blob/ba4081a5a8573875fed17545846f6f6902c8ba8d/src/libstd/io/timer.rs#L95-L101).
