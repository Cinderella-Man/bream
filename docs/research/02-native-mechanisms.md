# Native Rust preemptive tiny processes: feasibility and architecture obligations

Research date: **2026-10-07**. Scope: **native Rust process code, library embedding, preemptive scheduling, no application-authored yield calls**. Wasm is comparative evidence only. This report does not recommend replacing the requirement with Wasm or Elixir. It is a source-grounded investigation; no runtime experiment was performed for this report, and the original laboratory was read only.

Observed versions: Erlang/OTP **29.1.1**, Tokio **1.53.2**, corosensei **0.3.4**, Wasmtime **49.0.2**. Rust language and standard-library links are rolling documentation; the local executed probes separately record their compiler versions. Go's runtime page is a rolling source snapshot retrieved on the research date; Go 1.14 release notes are cited only for the historical introduction of asynchronous preemption.

## Finding

**Later requirement:** safe forced termination with runtime-managed cleanup is now essential. Recommendations below to defer cancellation apply only to the earlier scheduling-only question and do not satisfy the full accepted product. See [the dedicated termination analysis](05-forced-termination.md) and [actual candidate cancellation paths](06-cancellation-candidates.md).

**A native preemptive Rust library is possible in a constrained execution environment. Existing native preemption research disproves a blanket “a library cannot do this” claim.** What is unproven is the desirable combination of all these properties at once: arbitrary ordinary Rust and its full dependency ecosystem, safe public APIs, no compiler integration, no manual yields, very small suspended processes, and transparent cross-platform operation.

The appropriate project is a native process kernel with an explicit platform and compatibility contract. Investigate **signal-driven native stack switching** and **kernel-thread handoff** as separate implementation candidates. Keep automatic compiler instrumentation as a possible route to broader supported code, rather than assuming it is mandatory from the beginning. A successful infinite-loop demo proves scheduler control; it does not establish compatibility with allocators, TLS, locks, unwinding, blocking libraries, or arbitrary Rust dependencies.

The recommendation is a **native feasibility prototype first**, targeting Linux x86_64, with bounded capabilities and a deliberate inventory of supported standard-library behavior. The library's public shape can be `spawn(move || ...)`, ordinary blocking-looking process operations, join, and process-local data. It should not become an actor framework merely to demonstrate scheduling.

## Precise distinctions that avoid designing the wrong runtime

These are proposed terms for the project.

| Term | Proposed meaning |
|---|---|
| Native process | A native Rust execution context with its own stack and runtime identity; initially it shares the application's address space and allocator unless stated otherwise. |
| Preemptive scheduling | The runtime can deschedule a CPU-bound process that never calls a runtime API, then resume it. |
| Scheduling safe point | A state in which suspension/resumption preserves runtime and language invariants; it may be discovered asynchronously or inserted automatically. |
| Voluntary suspension | A process operation such as receive, sleep, or join that blocks the process without blocking its worker. |
| Worker | A scheduling capacity slot, which need not permanently equal one kernel thread. |
| Cancellation | A stop request with separately defined cleanup behavior; not a prerequisite for preemptive scheduling. |
| Forced termination | Abandoning live execution irreversibly. This is substantially stronger than pausing and resuming it. |
| Tiny | A measured resident-memory/density target including suspended stacks, metadata, queues, TLS, and any backing kernel threads. |
| Standalone | Embeddable independently of the laboratory's actors, cluster, upgrade, persistence, or service architecture. |

A native process can satisfy the scheduling goal without supplying Erlang-style memory isolation or an immediate safe `kill`. The user now explicitly requires the latter, so scheduling-only evidence is insufficient for the complete product.

## Why wrapping Future does not solve native preemption

`Future` exposes polling and waking, not a continuation capture primitive for an instruction currently executing inside `poll`. Consider `async { loop { std::hint::spin_loop(); } }`: a wrapper waiting outside the poll cannot regain control merely by checking its own budget. Also, `.await` does not imply a yield when awaited work is immediately ready. [Rust Future contract](https://doc.rust-lang.org/std/future/trait.Future.html)

Tokio deliberately schedules cooperatively. Its fairness assumptions include bounded poll duration and bounded task count; equal scheduling frequency is not promised. Those contracts explain why a new scheduler is needed for the requested CPU-bound behavior. They do **not** prove that signal- or OS-assisted native scheduling cannot be implemented in a library. [Tokio task model](https://docs.rs/tokio/1.53.2/tokio/task/index.html), [Tokio fairness assumptions](https://docs.rs/tokio/1.53.2/tokio/runtime/index.html#detailed-runtime-behavior)

## Candidate A: signal-driven fibers, with a narrow native contract

An external tick interrupts a running worker; the runtime preserves its machine context, picks another native process, and eventually restores the interrupted context. This delivers the actual requested behavior for a compute loop without source-level yield calls. The unsolved part is the environment in which a context switch is permitted, not whether registers can be saved.

Published native work gives concrete precedent. Shenango provides Rust closure-based bindings and signal-driven native user-thread preemption. Its implementation defers preemption in scheduler/RCU/spinlock/per-thread critical sections. The paper explicitly discusses cached TLS addresses and requires disabling preemption around TLS access, including glibc allocation calls. Thus the scheduling mechanism is real, while integration with ordinary native libraries is a documented constraint. [Shenango NSDI 2019, §6.2](https://joshfried.io/assets/pdf/shenango_nsdi19.pdf)

Proposed first contract:

- Linux x86_64 only, with an explicitly tested libc/kernel/toolchain configuration.
- Stackful native execution, fixed reserved stack regions plus guards; no relocating a live native stack.
- Pin a live process to one kernel thread initially. This removes an entire class of migration issues; it does not settle same-thread reentrancy.
- Runtime-owned process-local storage, synchronization, timers, and blocking-looking I/O operations.
- CPU-only code can be interrupted without runtime calls. Runtime internals enter bounded non-preemptible regions automatically.
- Unsupported blocking/foreign operations cross a named boundary whose scheduling behavior is documented.
- No forced stack discard, and no claim of isolation from unsafe memory corruption.
- The first low-level API remains experimental/unsafe where callers must uphold restrictions that Rust's types cannot enforce. A safe `spawn` is a milestone to justify, not a label to add optimistically.

“No manual yield calls” remains satisfied if the runtime/allocator/toolchain handles its own critical sections automatically. Requiring users to scatter yielding calls through compute loops would fail the requirement. Requiring users to follow an explicit initial library-compatibility contract is a separate restriction that should be evaluated candidly.

### Obligations before Candidate A can be called safe

The following table is an engineering checklist derived from the cited interfaces and runtime precedents. It is not a claim that every item requires a custom compiler.

| Obligation | Failure if omitted | Plausible first strategy |
|---|---|---|
| Complete interrupted CPU state | Corruption appears only under optimized SIMD/floating-point workloads | Use the platform signal frame/context and validate all supported register state; do not assume a cooperative callee-saved-register switch is sufficient |
| Signal stack ownership and nesting | A later signal overwrites a suspended handler | Explicit per-worker/per-context signal-stack policy and tested nested delivery |
| Reentrant scheduler entry | Signal interrupts queue mutation, lock ownership, or allocation | Very small nonallocating handler; bounded preemption-defer counter around runtime internals |
| Allocator compatibility | Interrupted allocator state is re-entered by another process | Controlled allocator boundary or validated allocator integration; measure deferred-preemption duration |
| TLS correctness | Migration changes thread-local identity; same-thread interleaving observes partial state | Pin initially and supply process-local storage; separately audit all ordinary TLS use |
| Native lock behavior | Blocking OS mutex pins worker while lock holder is suspended | Runtime-aware mutexes/channels or an explicit offload/handoff path |
| Stack exhaustion | Corruption or whole-application abort instead of process-level failure | Guard pages, conservative limits, tested overflow policy; do not promise safe recovery prematurely |
| Unwinding | Panic or destructor crosses unsupported assembly/signal frames | Catch at a normal process entry boundary; defer preemption across unwinder-sensitive regions |
| Ambient signal users | Profilers/crash handlers/timers conflict with runtime state | Document signal ownership; detect conflicts; define coexistence rather than globally replacing handlers silently |
| Library boundaries | CPU loop in a dependency lies outside the promised preemptible region | Audit/instrument supported dependencies, or explicitly narrow the contract |

Linux's signal-safety documentation distinguishes thread safety from asynchronous reentrancy and describes unsafe interrupted library-state access. It also cautions about leaving an interrupted unsafe function via `longjmp` and later calling unsafe functions. Moving code onto a scheduler stack does not, by itself, erase the interrupted library state. [Linux signal-safety API documentation](https://man7.org/linux/man-pages/man7/signal-safety.7.html)

Linux added `SS_AUTODISARM` specifically to support switching away from an alternate-stack signal handler without a subsequent handler corrupting it. This is an example of why a Linux-specific contract can be credible while “portable POSIX fibers” hides necessary details. The facility is one building block, not proof of a complete scheduler. [Linux sigaltstack API documentation](https://man7.org/linux/man-pages/man2/sigaltstack.2.html)

## Candidate B: kernel-thread handoff, preserving interrupted native state

The PPoPP 2021 paper *Lightweight Preemptive User-Level Threads* presents both signal-yield and **KLT-switching**. In the latter, preemption parks the entire underlying kernel thread and assigns the logical worker another kernel thread. The interrupted process later resumes on its original kernel thread, preserving its kernel-thread-local state. No compiler changes are required. A pool provides replacements; kernel-thread creation must happen outside the signal handler. The paper explains why a handler must not wait for creation if the interrupted code may hold allocator/global locks. In the worst case, the design grows to one kernel thread per user thread. [PPoPP 2021 paper, §3](https://sshiina.gitlab.io/papers/ppopp21.pdf)

This is a serious alternative, especially if ordinary Rust/library compatibility matters more than worst-case density. It avoids making newly scheduled arbitrary code run atop the *same interrupted kernel thread's* TLS state. It does not make dropping a live process safe, and it does not automatically solve logical deadlocks caused by shared locks.

Engineering questions for a Rust implementation:

1. Can all kernel-thread handoffs be implemented with a preallocated finite pool and async-signal-safe handshakes?
2. What happens when the pool is exhausted: defer this preemption, temporarily oversubscribe, reject admission, or expose a configurable reserve?
3. Can ordinary runtime waits release the kernel-thread association at a safe boundary while retaining Rust's thread-affinity assumptions?
4. Does the intended workload spend most of its lifetime waiting through runtime APIs, or spinning with live native state? The latter can destroy the density advantage.
5. Is the kernel-thread count a documented resource limit and an exposed metric?

Potential product shape: cheap many-to-many execution during normal runtime-managed waits, with temporary kernel-thread pinning when asynchronous preemption preserves unknown native state. The compatibility and resource tradeoff must be measured. A finite pool with unbounded runnable processes cannot simultaneously promise unrestricted immediate handoff for every interruption; the exhaustion policy is part of the public contract.

## Candidate C: automatic compiler/runtime safe points

Go 1.14 introduced asynchronous goroutine preemption, including loops without calls. [Go 1.14 release notes](https://go.dev/doc/go1.14#runtime)

Go's current implementation demonstrates why that feature is more than a timer signal. Synchronous safe points use stack checks; asynchronous interruption checks runtime state, stack space, and compiler metadata before redirecting to the register-saving preemption path. `isAsyncSafePoint` consults `PCDATA_UnsafePoint`, rejects unknown/native code and relevant runtime functions, and handles restartable instruction sequences. The scheduler is not simply run as an ordinary allocating routine in the signal handler. [Go runtime source](https://go.dev/src/runtime/preempt.go)

For Rust, automatic inserted checkpoints can honor “no manual yields” while keeping the scheduler in charge. This could use a supported constrained build pipeline, source transformation, or compiler integration. It must account for optimization, dependency code, std/allocator routines, loop backedges, recursion, and large noninterruptible operations. Instrumenting only the function carrying a procedural macro is not a general guarantee about callees.

This route has distinct advantages: explicit suspension boundaries, easier control of runtime critical sections, and an auditable account of where execution may move. Its cost is toolchain maintenance and potentially restricted dependency builds. Rust does not need Go's GC machinery merely to schedule native code; copy the principle of compiler/runtime agreement, not every Go implementation detail.

The key design question is whether a Cargo-installed library plus supported build integration still fits the intended standalone experience. That is a product boundary, not something to settle by quietly substituting a Wasm guest format.

## Why Send, TLS, and Drop matter even for safe Rust code

Corosensei explicitly does not implement `Send` for its stackful coroutine: the type system cannot prove that every live value on the stack is `Send`. Its force-reset operation is unsafe if objects on the stack need `Drop`; its ordinary unwind path starts from a controlled suspension point. This is direct evidence that a closure's initial bounds are not sufficient to reason about an interrupted live stack. [corosensei Coroutine](https://docs.rs/corosensei/0.3.4/corosensei/struct.Coroutine.html)

For example, `FnOnce + Send` can create a `MutexGuard` *after* it starts. The standard guard is deliberately non-`Send` because relevant platforms require unlocking on the acquiring thread. Likewise, a `Send` future can temporarily create non-`Send` values inside a poll and destroy them before returning. Those values cannot be asynchronously moved to a different worker merely because the suspended future was originally `Send`. [MutexGuard contract](https://doc.rust-lang.org/std/sync/struct.MutexGuard.html#impl-Send-for-MutexGuard%3C'_,+T%3E)

Thread-local storage belongs to the OS-thread execution environment; it is not automatically process-local when several green processes share that thread. Pinning prevents TLS identity from changing through migration, but asynchronous interleaving can still expose temporary thread-local state to another process on the same thread. Auditing migration and auditing reentrancy are two separate jobs. [Rust thread-local storage](https://doc.rust-lang.org/std/thread/index.html#thread-local-storage)

These observations rule out an *unjustified safe API*, not native preemption itself. A runtime can choose controlled boundaries, remain pinned, isolate unknown native states with KLT handoff, or change the generated-code contract. The investigation should measure those alternatives instead of declaring all of them impossible.

## Preemption is not forced cancellation

Pause-and-resume preserves a process's obligations. Throwing away a stack may leave locks held, borrows logically active, external operations outstanding, and destructors unexecuted. Running arbitrary destructors from a timer signal is a different unsafe operation, not a natural extension of switching stacks.

A scheduling-only native API could permit joining, voluntary stop requests at runtime operations, and ordinary panic observation without safe forced kill. That narrower API would not satisfy the user's subsequently accepted termination requirement. The full product needs the stronger generated/controlled execution and cleanup analysis in the companion report.

`catch_unwind` catches only unwinding panics; an aborting panic is outside it. Tokio likewise cannot abort a started `spawn_blocking` job. These are useful examples of the boundary between task scheduling and safe native-work destruction. [Rust panic catching](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html), [Tokio JoinHandle abortion](https://docs.rs/tokio/1.53.2/tokio/task/struct.JoinHandle.html#method.abort)

## A latency target is not a timer setting

Publish measured operational latency under a named platform/load profile. A 1 ms timer does not establish a 1 ms scheduling bound. A useful explanatory decomposition is:

`service delay = signal/timer delay + deferred-preemption time + scheduler queueing + native blocking time + OS scheduling interference`

This is a reasoning aid rather than a fitted equation or proof. Deferred regions may include allocator calls and unwinding; kernel-thread-pool pressure affects handoff; runnable population affects queue delay. A loop can be preemptible yet latency can still be poor under oversubscription.

Record separate results for quantum overrun, heartbeat latency, time to first scheduling of a newly ready process, queue fairness, and stop-request completion. Do not market one aggregate “preemption latency” as all five.

## The first experiments should try to break the native contract

1. Two native computation loops on **one scheduling worker**, neither calling the runtime. Verify both progress, including an optimized loop with no function calls.
2. Register-preservation torture: floating-point/vector calculations across frequent random interruption; compare against deterministic nonpreempted output.
3. Allocation/deallocation and TLS access under interruption, including allocator configuration variations. Test both pinning and any proposed migration mode separately.
4. Standard locks, runtime-aware locks, contended locks, thread IDs, `errno`, and dependency code that uses TLS internally.
5. Panics and destructors while other processes continue. Test `panic=unwind` explicitly; document the abort configuration separately.
6. Blocking syscalls, signals interrupting syscalls, profiler/handler coexistence, and alternate-stack nesting.
7. Stack-depth/overflow behavior and teardown; no test should rely on silently abandoning a live Rust stack.
8. Density across idle, I/O-waiting, and always-runnable processes, measuring resident memory, reserved address space, and **kernel-thread count**.
9. For KLT handoff, deliberately exhaust replacement threads and verify the documented degradation behavior.
10. A minimal application using only spawn, computation, join, and timer/receive; no actors, cluster, or persistence package.

A safe API claim additionally needs a written unsafe-code argument and independent review. Stress tests can reveal failure; passing them does not prove arbitrary Rust-code compatibility.

## What should be the standalone library boundary?

The smallest compelling product is a process execution kernel: spawn/join, native stack management, scheduling/preemption, process-local state, runtime-aware waits/synchronization, and observability. Mailboxes can be an optional primitive; an `Actor` trait, supervision trees, serialization, distributed naming, and hot upgrade belong above it.

Start with one backend and name its restrictions. Avoid a public “generic executable” trait whose contract merely *asks* implementations to return promptly while the project advertises enforced preemption. Any generic layer must preserve which guarantees each backend actually provides. Safe process spawning, unchecked low-level spawning, and trusted offload should not look interchangeable.

Measure tiny-process cost as: private stack pages + reserved stack region + process metadata + TLS/process-local data + queue/handle allocations + any suspended kernel-thread resources. Do not compare a native stackful process with a Tokio future's task-header size, or with an empty BEAM process's reported memory, without including the execution state each retains.

## What BEAM actually gives us

OTP 29.1.1 defines `CONTEXT_REDS` as **4000**, and the default minimum heap-plus-stack as **233 words**. These are implementation constants, not wall-clock service guarantees or universally stable API promises. The x86 JIT's yield instruction decrements the runtime's function-call counter and transfers to its yield path. Native machine code generated by BeamAsm therefore remains runtime-controlled code. [Pinned constants](https://github.com/erlang/otp/blob/OTP-29.1.1/erts/emulator/beam/erl_vm.h#L50), [pinned JIT yield implementation](https://github.com/erlang/otp/blob/OTP-29.1.1/erts/emulator/beam/jit/x86/instr_common.cpp#L3216)

BeamAsm converts BEAM instructions to native code while preserving much of the existing runtime arrangement. “BEAM has a JIT” consequently does **not** establish that unmodified `rustc` machine code can be dropped into its scheduling machinery. That equivalence would require evidence about calling conventions, suspension points, stack representation, runtime operations, and memory management which an ordinary Rust closure does not provide. [BeamAsm documentation](https://www.erlang.org/doc/apps/erts/beamasm.html)

BEAM's memory organization also matters: processes own stacks and heaps; collection is primarily per-process, with global large-object machinery. That provides a credible template for reclaiming a process's resources without traversing arbitrary shared application ownership graphs. It is not evidence of an OS address-space boundary. [ERTS garbage collector](https://www.erlang.org/doc/apps/erts/garbagecollection.html)

The efficiency guide's example reports **327 words / 2616 bytes** for a fresh idle process. Its example banner is older than the current document version; it is an illustrative baseline, not a current cross-platform guarantee. Its local-send explanation also matters: data is generally copied, with exceptions such as reference-counted binaries and literals. Density and isolation depend on the representation and ownership discipline, not the mere existence of a mailbox API. [Erlang process efficiency guide](https://www.erlang.org/doc/system/eff_guide_processes.html)

### The native-code exception is part of the BEAM lesson

Normal NIFs should usually return within about **1 millisecond**. Long work can be split, explicitly rescheduled, sent to another thread, or marked dirty for separate CPU/I/O schedulers. Dirty classification does not make native work interruptible. OTP explicitly states that terminating a process running a dirty NIF can release Erlang resources and trigger links/monitors **while the NIF continues executing**; some internal reclamation waits for its return. Native faults can damage the VM. [OTP NIF manual](https://www.erlang.org/doc/apps/erts/erl_nif.html#long-running-nifs)

Design consequence: copy BEAM's *separation of runtime-controlled computation and foreign work*, including the uncomfortable exceptions. An API called `kill` cannot honestly imply “no more external effects are possible” merely because the process disappeared from the process table.


## Wasmtime is comparative evidence, not the proposed product

Wasmtime shows another way to make mandatory suspension points usable behind a Rust API. Its async calls produce futures and use separate stacks; fuel can account computation and yield periodically, while epoch deadlines can yield or trap. They affect guest execution, not arbitrary synchronous host code. Bulk memory operations and host calls illustrate why even controlled-code engines have granularity limits. [Wasmtime async model](https://docs.rs/wasmtime/49.0.2/wasmtime/index.html#async), [released Store source](https://github.com/bytecodealliance/wasmtime/blob/v49.0.2/crates/wasmtime/src/runtime/store.rs#L981), [released configuration source](https://github.com/bytecodealliance/wasmtime/blob/v49.0.2/crates/wasmtime/src/config.rs#L678)

The checked-out laboratory Lunatic revision, **28a2f387ebf6a64ce4b87e2638812e2c032d5049**, uses Wasmtime major **8** and explicitly depends on periodic `Poll::Pending` for signal handling. Its architecture is useful comparison material, but extracting its Wasmtime-backed execution core would not satisfy the corrected native-Rust requirement. [Pinned Lunatic runner](https://github.com/lunatic-solutions/lunatic/blob/28a2f387ebf6a64ce4b87e2638812e2c032d5049/crates/lunatic-process/src/lib.rs#L266), [engine adapter](https://github.com/lunatic-solutions/lunatic/blob/28a2f387ebf6a64ce4b87e2638812e2c032d5049/crates/lunatic-process/src/runtimes/wasmtime.rs#L39)

## Reuse and licensing

Borrow BEAM's principles: a coherent execution unit, runtime-owned scheduling boundaries, small initial execution state, process-local ownership, observability, and a clearly marked boundary around native work. Its scheduler's references to ERTS process structures, reductions, JIT conventions and GC state argue against extracting a few C scheduling files as a generic native-Rust solution.

OTP 29.1.1's top-level license is **Apache-2.0**. Its terms permit reuse subject to their conditions, including license/notice preservation and marking modified files where applicable; individual bundled components need their own provenance review. A Rust crate's MIT/Apache choice does not erase copied upstream notices. Prefer recording source revision and attribution for any copied code rather than vaguely saying “stolen from BEAM.” This is a source-license observation, not a complete legal audit. [Pinned OTP license](https://github.com/erlang/otp/blob/OTP-29.1.1/LICENSE.txt)
