# Landscape: an embeddable runtime for preemptible tiny processes

Research snapshot: 2026-10-07. Primary documentation, repository source, GitHub API and crates.io API were checked live. This investigation did not build or execute the external projects. The parent lab's recorded experiments and the companion native-preemption experiment are separate evidence. Architectural recommendations below are inference, not existing implementation guarantees.

**User clarification, incorporated:** native Rust processes that automatically and resumably yield CPU are essential. Wasm guest compilation and programmer-inserted yields do not satisfy the requested product. The Wasm entries below are prior art and alternative semantics, not the recommended implementation. Native runtime internals or compiler instrumentation remain possible research routes; neither can silently narrow the public promise without saying so.

## What opportunity actually remains?

**Later requirement:** the user also requires safe forced termination and has accepted investigating a restricted generated process model under ordinary Cargo. This landscape's scheduling evidence does not qualify kill/cleanup. See [candidate cancellation audit](06-cancellation-candidates.md) and [generated model](../generated-process-model.md).

The desired product is a small Rust library that owns **resumably scheduled native execution contexts**, composes with an existing application, and makes actors optional. It would be closer to an embeddable process kernel than another general I/O executor. Resource accounting and failure isolation must be described separately: a native private stack does not imply a private heap or a memory-safety sandbox.

There were two materially different execution boundaries hiding in the initial “like Tokio” analogy; the user has now required the native boundary:

1. A native Rust application adds a dependency and embeds a runtime whose **guest** code is separately compiled to Wasm. This can deliver automatic continuation-preserving switches for CPU loops using an existing engine. It does not accept arbitrary native closures and all their existing dependencies unchanged.
2. A native Rust application adds a dependency and runs **native** computations with automatic interruption and resumption. This is the accepted direction. Compatibility with arbitrary dependencies, cross-worker migration, forced cancellation, and cross-platform support are separate open choices, not additional requirements inferred from the user's answer.

“A library” and “a VM” are not opposites. Wasmtime and Lunatic are embeddable libraries. “Independent of the actor platform” and “independent of Tokio internally” are also different requirements. A process library can use Tokio internally or offer a Tokio host adapter without requiring applications to adopt a whole actor system.

The non-negotiable behavior must be tested as **suspend, run another process, resume the same computation with its live locals intact**. Killing an over-budget job and later starting a replacement is a different result.

## Competitors and reusable mechanisms

| Candidate | Application integration and execution boundary | Relevant guarantee | Consequence for extraction |
|---|---|---|---|
| Tokio | Native `Future` tasks; regular Cargo library | Cooperative budget at participating operations; no forced interruption of arbitrary non-returning `poll` | Excellent I/O/host dependency and fairness baseline; cannot supply the central CPU-loop contract by adding a wrapper |
| Lunatic | Embeddable Rust host library **and** command-line runtime; guest modules compiled to Wasm | Already offers preemptively scheduled Wasm processes, isolation and actor services | Closest Wasm architectural prior art; does not satisfy the selected native product |
| Wasmtime directly | Rust library hosting separately compiled Wasm modules/components | Fuel/epoch interruption can either trap or asynchronously yield and later resume | Useful fairness/semantics baseline; not the native execution mechanism requested |
| May | Native stackful closures and coroutine-aware I/O/synchronization APIs | Cooperative execution; explicit long-CPU-loop yields; documented TLS restrictions | Useful ergonomics and stack-cost comparator, not an automatic CPU-preemption solution |
| corosensei | Native stack-switching primitive | Explicit suspend/resume, controlled unwind support; no process scheduler | Possible mechanism inside a restricted native experiment; does not supply transparent preemption or arbitrary stack migration |
| Glommio | Native futures, thread-per-core I/O executor with queues/shares | Cooperative `yield_if_needed`; its preemption timer changes when cooperation is requested | Useful scheduling-policy reference; timer terminology does not mean it interrupts arbitrary instructions |
| Hydra | Native Tokio-based processes, OTP-like actor API and distribution | Host actor semantics, not forced CPU-loop scheduling | A useful upper-layer comparator; extracting its scheduler preserves Tokio's native execution limitation |
| open-coroutine | Native stackful library, signal/context machinery and syscall hooks | Explicitly claims CPU-loop preemption; current source contains a resumable native mechanism | Serious native counterexample to audit, not grounds for saying this is impossible; broad safety and composition remain unestablished |
| rrkernel | Native closures; Linux x86-64/glibc fibers, Windows OS threads, plus hardware RTOS ports | Automatic native preemption mechanisms; desktop guarantees differ materially from hardware timing | Linux is relevant native prior art; Windows does not demonstrate tiny M:N processes; hardware density/timing cannot be transferred to desktop |

Tokio's cooperative module explicitly explains starvation from a long `poll` and the need for participating yield points. A wrapper cannot interject once a poll runs indefinitely. [Tokio coop documentation](https://docs.rs/tokio/1.53.2/tokio/task/coop/index.html).

Lunatic's README describes private stacks/heaps and preemptive scheduling on a work-stealing async executor. Its library documentation explicitly describes embedding the VM inside Rust, with a separate SDK for compiling guest actors. Its manifest exposes a library target and several process/API crates. Therefore **“Lunatic is only a separate binary” would be factually wrong**. [Lunatic README](https://github.com/lunatic-solutions/lunatic/blob/28a2f387ebf6a64ce4b87e2638812e2c032d5049/README.md), [embedding library](https://github.com/lunatic-solutions/lunatic/blob/28a2f387ebf6a64ce4b87e2638812e2c032d5049/src/lib.rs), [manifest](https://github.com/lunatic-solutions/lunatic/blob/28a2f387ebf6a64ce4b87e2638812e2c032d5049/Cargo.toml).

Wasmtime distinguishes a non-resumable trap from an async yield that preserves execution for later continuation. Both fuel and epoch mechanisms can support that yield mode. Fuel is a deterministic instrumented work budget under a fixed execution configuration, not elapsed CPU time. Epochs trade determinism for lower overhead and require the embedding to drive the clock. Neither creates a process scheduler or automatically makes native imports bounded. [Wasmtime interruption documentation](https://docs.wasmtime.dev/examples-interrupting-wasm.html).

Glommio documents `yield_if_needed` as its central cooperation mechanism. `LocalExecutorBuilder::preempt_timer` configures the condition observed by `need_preempt`; it does not establish asynchronous interruption of a Rust instruction stream. [Yield API](https://docs.rs/glommio/latest/glommio/fn.yield_if_needed.html), [executor builder](https://docs.rs/glommio/latest/glommio/struct.LocalExecutorBuilder.html#method.preempt_timer).

Hydra remains an actor-platform comparison rather than a new execution boundary. The original lab already tested saturated native computation, finding stalled progress. Its source uses Tokio task spawning. Treat the lab's recorded result as stronger evidence for that pinned version than feature descriptions. [Hydra process source](https://github.com/dtzxporter/hydra/blob/37c630fc77316049a186907d87dfc47109777433/hydra/src/process.rs), [lab foundation report](/home/bat/projects/rust-actor-runtime-lab/docs/foundation-recommendation.md), [lab failure report](/home/bat/projects/rust-actor-runtime-lab/docs/actor-failures.md).

## Native preemption: mechanism versus public guarantee

### May is cooperative, and its caveats are instructive

May's own caveat document says CPU-heavy code should call `coroutine::yield_now`, blocking APIs park the worker, and coroutine-aware replacements may be required for dependencies. It explicitly discusses TLS hazards. The current `spawn` is an unsafe API, even though `F: Send` and `T: Send`; a convenient spawning macro does not erase the underlying obligations. Do not describe May as preemptive merely because it has stackful coroutines and cancellation. [May caveats](https://github.com/Xudong-Huang/may/blob/842d98383e6d84227fa6ebd87e0927a01147ede5/docs/may_caveat.md), [May spawn contract](https://docs.rs/may/0.3.51/may/coroutine/fn.spawn.html).

### corosensei explains the hidden stack problem

corosensei's `Coroutine` deliberately does not implement `Send`: the entry closure's properties cannot prove that every value subsequently created and left on the suspended stack is `Send`. It allows manually establishing a stronger invariant when all executed code is under the caller's control. It also distinguishes resumption, unwinding on drop, and unsafe `force_reset`, whose contract excludes live stack objects requiring destructor execution. These are separate proof obligations. [Coroutine contract](https://docs.rs/corosensei/0.3.4/corosensei/struct.Coroutine.html).

Inference: a native process runtime should initially pin each started stack to one worker unless it can justify migration of **all** suspended stack state, not merely require `Send` on the spawn closure. Pinning removes one issue, but it does not isolate thread-local state between processes running on that same worker, solve library reentrancy, or make cancellation safe.

### open-coroutine is a real native mechanism to examine

The current upstream monitor uses `SIGURG` on Unix. Its handler checks the coroutine state, adjusts the signal mask, and calls the current suspender. It intentionally avoids preempting a coroutine in its `Syscall` state; comments describe a race fix to prevent a suspended syscall becoming stranded without a wakeup. Windows has separate thread-suspension/context-redirection machinery. The monitor docs describe a roughly 10 ms run threshold, not a real-time deadline. This is substantially more than a cooperative yield macro. [Monitor source, pinned](https://github.com/acl-dev/open-coroutine/blob/d725ceab4b780d57c9aa88777a96742e31549914/core/src/monitor.rs), [monitor documentation](https://github.com/acl-dev/open-coroutine/blob/d725ceab4b780d57c9aa88777a96742e31549914/core/docs/en/monitor.md).

Its hook documentation warns that using the core's preemption directly without the hook machinery can have “catastrophic consequences.” The wrapper links a hook dynamic library and redirects supported syscalls. That is important integration scope: users are adopting behavior around existing native calls, not just a private run queue. [Hook contract](https://github.com/acl-dev/open-coroutine/blob/d725ceab4b780d57c9aa88777a96742e31549914/hook/docs/en/hook.md).

The project does account for some lock problems: its `pthread_mutex_lock` adapter uses try-lock and event waiting while inside a coroutine. One should acknowledge this mitigation rather than assume it naively blocks every mutex. That hook alone does not establish coverage of arbitrary synchronization. Rust has a futex-based standard mutex implementation, and custom allocators/FFI libraries have their own paths. [Mutex hook](https://github.com/acl-dev/open-coroutine/blob/d725ceab4b780d57c9aa88777a96742e31549914/core/src/syscall/unix/pthread_mutex_lock.rs), [Rust futex mutex source, 1.90.0 example](https://github.com/rust-lang/rust/blob/1.90.0/library/std/src/sys/sync/mutex/futex.rs).

One concrete cancellation concern appears in current source: unfinished coroutine destruction calls `unsafe { self.inner.force_reset() }`. The upstream corosensei contract requires the absence of live stack objects needing `Drop`. The wrapper's general closure constructor does not itself impose that restriction. This deserves an explicit destructor/borrow-lifetime audit before relying on kill/drop semantics. It is **not** necessary to resolve cancellation in order to demonstrate pause/resume, but passing pause/resume tests would not validate cancellation. [Wrapper drop and constructor](https://github.com/acl-dev/open-coroutine/blob/d725ceab4b780d57c9aa88777a96742e31549914/core/src/coroutine/korosensei.rs#L380), [underlying force-reset contract](https://docs.rs/corosensei/0.3.4/corosensei/struct.Coroutine.html#method.force_reset).

The inspected public preemptive example mixes printing, sleep calls, and mutable globals. It should not be adopted unchanged as the proof of uninterrupted pure computation. A new qualification needs a finite arithmetic loop with no imports, no sleep and no explicit yield, verifies its final result, and records another process's progress before completion. [Example, pinned](https://github.com/acl-dev/open-coroutine/blob/d725ceab4b780d57c9aa88777a96742e31549914/open-coroutine/examples/preemptive.rs).

### What remains to prove for a native library

These are design/audit requirements, **not confirmed failures in every implementation**:

- **Signal and library reentrancy:** if process A is interrupted within an allocator or libc critical region and the scheduler/process B reenters that facility before A resumes, state corruption or deadlock can result. Scheduler allocation, TLS bookkeeping and signal handler nesting belong inside the proof. POSIX only promises a restricted set of async-signal-safe operations; bypassing a signal frame does not make arbitrary code reentrant. [Linux signal-safety manual](https://man7.org/linux/man-pages/man7/signal-safety.7.html).
- **Lock-held descheduling:** A can be suspended holding a lock that B then waits on. This is not automatically a deadlock; a correct runtime can resume A. It becomes one if B blocks the only runnable worker and the mechanism cannot interrupt or park B safely, or if the scheduler itself needs A's lock. Arbitrary FFI/custom locks make universal coverage harder than hooking named pthread calls.
- **TLS semantics:** multiple processes on one OS thread share native TLS, and migration changes thread identity. Thread-local handles, errno, recursive mutex ownership, compiler assumptions and foreign library state require a declared compatibility contract.
- **Stack migration:** a closure may create `Rc`, thread-bound guards or foreign handles after entry. An entry `Send` bound does not authorize moving that suspended continuation.
- **Panic and cancellation:** catching a Rust panic does not contain aborts or memory corruption. Forced cancellation must establish cleanup and borrowing invariants independently from context switching; unwinding arbitrary interrupted foreign frames is not automatically valid.
- **Embedding coexistence:** signal handlers, panic hooks, syscall interception and profilers must coexist with an application that also uses Tokio, Wasmtime or other runtimes. Global replacement hooks are a meaningful library adoption cost.
- **Platform/compiler coverage:** a Linux x86-64 demonstration is valuable evidence for that target, not evidence for all native Rust targets, optimizations, libc implementations or sanitizers.

Conclusion: **native resumable preemption exists; transparent, safe, portable preemption of arbitrary unchanged Rust code has not been established by this survey**. The user wants the native product, so that is the research direction. Existing signal runtimes and a compiler-assisted route must be compared on declared native compatibility, not displaced by a Wasm implementation.

### rrkernel: distinguish three execution models

The released 0.3.0 is a native preemption counterexample worth retaining. Its README records host tick/parking defects and describes multicore as unsupported, alongside successful microcontroller timing measurements. Those measurements are evidence about specific hardware, not latency or density promises for a Rust desktop library. [rrkernel 0.3.0 documentation](https://docs.rs/crate/rrkernel/0.3.0).

The Linux backend is explicitly Linux/glibc x86-64, with all task stacks on the calling OS thread, hand-written switching, and a SIGALRM timer. It avoids a shared alternate signal stack because suspended handler frames must remain on their respective fiber stacks. It raises small stacks to at least 8 KiB plus frame space. Kernel critical sections mask the timer signal. This is a relevant single-worker native baseline, not yet an M:N implementation. [Linux backend](https://github.com/AhmedBoin/rrkernel/blob/8075f6af0d0e1db66eafcaba40f31870576afdef/src/arch/posix.rs).

A specific embedding gate: release source and inspected HEAD select `SIGEV_SIGNAL`, which delivers to the process, with global scheduler state. Adding unrelated native threads requires an ownership/masking strategy so the handler executes on the designated scheduler thread. Linux also exposes thread-targeted timer delivery. This is source-derived risk, not a reproduced rrkernel failure. [Timer selection](https://github.com/AhmedBoin/rrkernel/blob/8075f6af0d0e1db66eafcaba40f31870576afdef/src/arch/posix.rs#L477), [Linux timer semantics](https://man7.org/linux/man-pages/man2/timer_create.2.html).

The Windows backend creates one suspended Win32 **OS thread per task** and controls it from a tick thread using suspend/resume. It sets a minimum 64 KiB requested stack and explicitly says OS thread count is the density limit. This may test a common scheduling API but does not prove a tiny M:N Windows runtime. [Windows backend](https://github.com/AhmedBoin/rrkernel/blob/8075f6af0d0e1db66eafcaba40f31870576afdef/src/arch/win32.rs#L595).

Sources were also read from the exact crates.io 0.3.0 source archive, not only the newer repository head. No build was executed in this landscape subtask. Allocation/libc/TLS, safe panic boundaries, handler coexistence, guard pages, stack exhaustion and teardown remain qualification topics. The public RTOS-style surface and globally configured kernel would need a concrete embedding audit before adopting it as a Tokio-composable library.

### A scheduling-only native contract and its limit

A scheduling-only research boundary is **Linux x86-64 with a declared glibc/toolchain matrix; one execution worker; native owned `'static` closures; fixed affinity for each started stack; bounded stack allocation and explicit overflow behavior; automatic slices; no arbitrary forced cancellation**. This does not satisfy the subsequently accepted first-product kill requirement. It remains a useful mechanism comparator. A thread-safe external submission/join handle can exist without migrating the process's stack; I/O and synchronization compatibility must be explicit.

That is a platform restriction and FFI contract, not a request for users to scatter yields in CPU loops. It also does **not** justify a safe all-Rust-code `spawn` API until the runtime proves arbitrary interruption cannot violate Rust/native library assumptions. If that proof fails for signal hijacking, the alternative is a controlled native build pipeline that inserts automatic checks and records safe suspension points across the transitive code it owns. Then uninstrumented FFI is an explicit boundary. The artifact remains native machine code; the build contract changes.

Two tracks should compete before choosing the kernel:

| Native track | What it buys | What must be demonstrated |
|---|---|---|
| Signal/context preemption with pinned stacks | Existing code can run without source-level yields; existing prototypes to study | Signal-safe context transfer and scheduler, no unsafe reentrancy, supported std/libc/TLS semantics, global-hook coexistence, correct unwind policy |
| Compiler-assisted native suspension | Automatic user-visible preemption at runtime-approved points; opportunity for stack maps and migration rules | Checks cover loops/call chains/dependencies after optimization, bounded distance between checks, native FFI escape policy, sustainable toolchain integration |

Do not announce cross-worker migration, BEAM-style memory isolation, safe arbitrary kill, transparent compatibility with every crate, or hard real-time slices as consequences of the first successful scheduling demo.

## Adopt, fork, or extract?

Lunatic already bundles much of the high-level Wasm-process idea. Its manifest still selects Wasmtime 8, whereas the source lab's modern Wasmtime probes pursue newer execution/resource APIs. A fork/modernization was a serious alternative under the original ambiguous execution requirement. Native execution is now a decisive distinction rather than merely a new name or facade. [Lunatic dependency manifest](https://github.com/lunatic-solutions/lunatic/blob/28a2f387ebf6a64ce4b87e2638812e2c032d5049/Cargo.toml).

The lab recorded concrete Lunatic build repairs and behavior gaps, but that supports a comparison rather than automatically making a rewrite cheaper. Reusing a compatible subset of Lunatic's process model may be less work than reconstructing it. Original lab findings should be carried over with pinned versions and failure scopes intact. [Lab Lunatic baseline](/home/bat/projects/rust-actor-runtime-lab/docs/research/05-lunatic-build-baseline.md), [lab foundation options](/home/bat/projects/rust-actor-runtime-lab/docs/foundation-recommendation.md).

Lunatic/Wasmtime modernization would address the separate guest-compilation product and therefore does not satisfy the clarified native goal. Preserve the lab's host-side lifecycle, admission and qualification ideas, but expect a new native execution mechanism rather than a rename of its Wasm adapter.

Recommended native evaluation: compare a narrow open-coroutine signal baseline, rrkernel's Linux mechanism, and a compiler-assisted native prototype against the **same three consumer programs**:

1. An ordinary native CLI running several CPU-heavy native jobs and awaiting completion, without an actor framework; describe memory as shared unless an additional isolation mechanism exists.
2. An existing Tokio service running long-lived per-session computations while its network/operator work remains responsive.
3. A typed actor adapter providing mailbox and supervision semantics on top of the same kernel.

If the first two require importing Kameo, OTP naming, the application's state schema, or migration coordinator code, the extraction boundary is still too high. If they merely expose unsafe context switching without a usable scheduling/lifecycle contract, the boundary is too low.

Reuse I/O machinery and existing low-level context mechanisms when their guarantees fit. A new scheduling kernel may be justified because the controlled native execution boundary is itself the missing capability. “Generic” should mean independently useful across those three consumers, not a proliferation of unproven backend traits.

## Minimal boundaries that keep actors optional

Suggested kernel responsibilities:

- `Runtime`/`RuntimeHandle`: explicit ownership, start/shutdown, resource admission, configuration, host integration.
- A native entrypoint and a live process incarnation: shared native code/address space, private stack and continuation, explicit borrowing/thread-affinity contract. A compiler-assisted mode must visibly declare its build requirements rather than pretend arbitrary uninstrumented functions satisfy the same promise.
- Spawn/join/exit notification; distinguish normal completion, trap, requested cancellation and runtime shutdown.
- Scheduling quantum and runnable/blocked state; execution yields preserve stacks and locals. Execution budget exhaustion policy is separate from an ordinary time slice.
- Accounting/admission across continuation stacks, queued inputs, managed resources and process count. Heap allocation accounting needs an additional policy/mechanism; private stacks alone do not supply it. “Tiny” is a measured budget.
- Minimal bounded communication or I/O attachment points, with explicit host resource ownership and cancellation semantics.
- Per-process metadata observable while computation is busy: status, resource use, runtime identity, reason for exit. Business-state inspection may still require process cooperation.

Keep typed actor protocols, selective receive, supervisors/restart strategy, stable logical names, remote nodes, distributed links, durable state, Elixir translation, message-boundary upgrades, fleet rollout and business schemas in optional layers. Some low-level death notification and identity semantics must exist in the kernel so an actor layer can be implemented correctly; a whole OTP behavior library need not.

A Tokio adapter is an integration layer: host futures may await native process results or bridge runtime-aware I/O. Native fibers should execute on their owned workers; wrapping an existing Tokio `poll` in a process handle does not by itself allow safely interrupting and migrating that poll.

## Qualification gates before committing to the product

1. **Real resumption:** finite pure arithmetic work exceeds many slices; unrelated process progress is observed; the original computation completes with correct locals/result. One execution worker avoids accidentally mistaking spare OS threads for scheduling fairness.
2. **No kill-as-yield:** verify the same process incarnation and native continuation survive scheduling switches. Separately test normal unwind, cancellation requests and termination policies.
3. **Foreign-call boundary:** long/blocking calls cannot silently bypass the declared fairness envelope. Native libraries need explicit restrictions, offload, scheduler-aware adapters, or proven interruption handling.
4. **Density:** measure RSS/PSS, reserved virtual address space, stack commitment, metadata, OS thread count and marginal cost at several population sizes. A private stack and a shared native heap are distinct costs.
5. **Generic consumer:** build both a CPU-job CLI and an actor adapter. This is evidence of a generic boundary, unlike designing a generic trait before a second consumer exists.
6. **Native compatibility:** constrain supported OS/architecture and stack migration policy first; test locks, allocation, TLS, blocking foreign calls, panic/unwind and destructor effects under repeated preemption. Preserve failure artifacts and compare against threads and cooperative coroutines.

Decisive falsification tests for the narrow contract: a CPU loop with no explicit yields and correct final checksum; at least two loop bodies interleaving on the same kernel TID; negative control with preemption disabled; TLS isolation/identity test; interruption during allocation and contended standard/custom mutexes; an unhooked FFI sleep and a native foreign CPU loop; nested signal delivery with existing handlers; all-SIMD-register/FP-state preservation; stack overflow at a guard page; destructor counters during normal return/panic/shutdown; and existing native/Tokio threads alive before runtime initialization. Run every candidate in an externally timed child process, under optimized builds as well as debug. Passing a stress test supports its tested configuration; it does not replace a safety argument.

## Version and maintenance snapshot

Registry `max_version` and creation dates were retrieved via each linked crates.io API on 2026-10-07. Main-branch SHA/date came from GitHub's repository and commit APIs. No release cadence or maintenance promise is inferred from age alone. All seven queried GitHub repositories were unarchived at inspection.

| Crate | Latest registry version observed | Release date UTC | Current inspected repository head/date |
|---|---|---|---|
| [tokio](https://crates.io/api/v1/crates/tokio) | 1.53.2 | 2026-10-03 | API docs inspected; no repo head recorded |
| [lunatic-runtime](https://crates.io/api/v1/crates/lunatic-runtime) | 0.13.2 | 2023-05-03 | `28a2f387ebf6a64ce4b87e2638812e2c032d5049`, 2025-03-29 |
| [lunatic-process](https://crates.io/api/v1/crates/lunatic-process) | 0.13.2 | 2023-05-03 | Same Lunatic workspace |
| [may](https://crates.io/api/v1/crates/may) | 0.3.51 | 2025-05-13 | `842d98383e6d84227fa6ebd87e0927a01147ede5`, 2026-08-03 |
| [corosensei](https://crates.io/api/v1/crates/corosensei) | 0.3.4 | 2026-05-24 | `4b5db4845204a4b2663d3f7f75866be4cc3c680d`, 2026-05-24 |
| [glommio](https://crates.io/api/v1/crates/glommio) | 0.9.0 | 2024-03-25 | `8434815962ce0bc161ace1967137213dc2334e4b`, 2025-04-21 |
| [hydra](https://crates.io/api/v1/crates/hydra) | 0.1.40 | 2025-08-23 | `37c630fc77316049a186907d87dfc47109777433`, 2025-08-23 |
| [open-coroutine](https://crates.io/api/v1/crates/open-coroutine) | 0.7.0 | 2025-05-28 | `d725ceab4b780d57c9aa88777a96742e31549914`, 2026-10-06 |
| [rrkernel](https://crates.io/api/v1/crates/rrkernel) | 0.3.0 | 2026-09-19 | `8075f6af0d0e1db66eafcaba40f31870576afdef`, 2026-09-22 |

Licenses from registry metadata: Tokio/Hydra MIT; Lunatic/May/corosensei/Glommio MIT or Apache-2.0; open-coroutine Apache-2.0. A code extraction still needs per-file/provenance review; this is metadata, not legal clearance. No newly fetched upstream code was copied into the original project.

Unknowns: no production density estimate, no representative application performance comparison, no complete native-runtime soundness audit, and no proof that a new native kernel is cheaper than strengthening an existing one. The guest-compilation boundary has been rejected for this requested product. Standalone native resumable scheduling is the target; broad crate compatibility remains to be established.
