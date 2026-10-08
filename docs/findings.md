# Findings: a native preemptive process library

## Verdict

**Implementation checkpoint, 2026-10-08:** five restricted milestones pass: [useful owned buffers](../experiments/process-milestone/README.md), [structured state/helpers](../experiments/structured-process/README.md), [identity/lifecycle](../experiments/identity-lifecycle/README.md), [mailboxes/parking](../experiments/mailbox-process/README.md) and [receive timeouts](../experiments/receive-timeout/README.md). The latest preserves scan/wait semantics and cancelled-timer identity, with an injected clock, bounded expiry service and ownership cleanup. Thirty-five unit tests per profile and six timeout invocations pass, including 96 send/timeout and 96 kill/timeout races. A missing nested-match checkpoint was exposed by a failing logical-budget control and fixed. The [progress view](milestones.md) tracks partial G1/G2/G3 and the next monitor/Down milestone; real-time timer driving remains open. No speed tests or comparisons may run here, and prior local timings are unsuitable performance indicators.

**Native preemptive scheduling is demonstrated, and a generated native subset also demonstrates safe stop/cleanup. The complete general-purpose library remains a research problem.** One fresh probe interrupts ordinary native Rust loops on one worker; a second uses mandatory generated checkpoints and normal destruction to stop an infinite process and reclaim its tracked buffer. The existing lab is a useful source of requirements and test scenarios, but it does not contain an engine satisfying the clarified native execution and kill contract.

The next engineering investment should establish an enforceable process-ownership and termination model before selecting the production engine. Existing native mechanisms remain useful comparators, but their scheduling success does not establish safe destruction. The research does not establish a safe general-purpose library, BEAM-equivalent density, or a production-ready engine. [Executed probe](../experiments/native-preemption/README.md), [scope decisions](design-tree.md).

## The product requirement is specific

The subsequent planning session explicitly selected **BEAM/ERTS primitives first**: typed process mailboxes, send/selective receive, monitoring, linking and exit/trapping behavior. The room/session example uses raw processes; GenServer and supervisor frameworks are outside the first scope. [Accepted boundary](adr/0004-beam-primitives-first.md), [current roadmap](roadmap.md).

The user asked for a Rust library independently useful like Tokio, with BEAM-like small processes that the runtime preempts. Subsequent answers make the initial contract explicit:

- Native Rust computations, not a separately compiled Wasm product or an Elixir execution environment.
- CPU loops are forcibly descheduled and later resumed without manual yield calls.
- Long-lived stateful services are the first workload.
- Runtime-aware synchronization and I/O are acceptable.
- Linux x86-64 is an acceptable first platform.
- Ordinary Cargo dependency and standard Rust compiler first.
- Safe forced termination is essential, with runtime-managed cleanup rather than guaranteed execution of arbitrary user destructors.
- Processes own their state and communicate through owned messages; shared mutable state across processes is not required.

“Preemptive” does not mean a timeout kills a task. The concrete behavior is A computes → runtime suspends A → B computes → A resumes with its live local state. Optional total CPU quotas are a different feature. Borrowing BEAM scheduling principles does not require importing its actor ecosystem wholesale.

## Fresh native evidence

The probe uses published `open-coroutine-core = 0.7.0`, Rust 1.98.1, Linux x86-64/glibc 2.43. Both computations run through one scheduler on one OS task-worker thread, with a monitor helper supplying interruption. Each executes 100 million arithmetic iterations. The loop body performs arithmetic and occasional atomic observations; it contains no yield, await, allocation, lock, I/O, sleep, clock read, or syscall.

| Observation | Preemption enabled | Disabled control |
|---|---:|---:|
| Both computations complete | Yes | Yes |
| Same task-worker thread for both | Yes | Yes |
| Both observe overlapping progress | Yes | No |
| Observed task-to-task changes | 25 | 1, serial first task then second |
| Final arithmetic checksums | Same | Same |

These are sampled task changes, not an exact interrupt count. The experiment verifies result preservation for this workload; it does not prove preservation of every possible SIMD/register/library state. Both binaries and final input hashes are retained, along with the expected failing control verdict. [Raw comparison](../evidence/native-preemption/comparison.json), [complete method](../experiments/native-preemption/README.md).

This is enough to reject “an ordinary native library cannot preempt a Rust loop” as a blanket claim. It is not enough to expose unrestricted safe `spawn` without further analysis. The upstream core explicitly relies on additional hook machinery for broader native operations, which this deliberately restricted probe does not include.

## What can actually be carved from the old project?

| Asset in rust-actor-runtime-lab | Relevance to Bream |
|---|---|
| Wasmtime `Guest` envelope duplicated across resident and portable fixtures | Good evidence of a separable execution responsibility; its engine/ABI does not implement the requested native product. |
| Single-worker CPU saturation and unrelated-progress probes | Reuse their assertion pattern with native computations. |
| Typed resident state separated from execution adapters | Preserve the domain/runtime separation. Native state should not require JSON, snapshots, or serialization to execute. |
| Trap, mutation, lifetime and supervision experiments | Reuse the questions, but derive native answers. Discarding a Wasm Store is not equivalent to discarding a native Rust stack. |
| Identity, stale reference and operator-control investigations | Useful process-lifetime and inspection requirements; not necessarily first engine code. |
| Kameo/Hydra actor lifecycle and network experiments | Higher-layer integration references. They do not supply native CPU preemption. |
| Upgrade queues, Elixir bridge, Phoenix Clock, distributed migration | Keep outside the process runtime. They can later consume the library. |

The lab's complete final evidence is credible within its limits: final source/binary hashes match, the portable run contains 495 passing observations, and two retained compiled migration tests were rerun successfully. That credibility must not be promoted into proof of native scheduling. [Detailed extraction audit](research/01-lab-extraction-audit.md), [hash record](../evidence/lab-evidence-check.json).

## What to borrow from BEAM

Borrow the coherent process abstraction and scheduling obligations: a runnable process gets another turn; waiting should not consume a worker; scheduling metadata is available without asking a stuck process to answer; identity refers to a lifetime; native operations that the scheduler cannot control need a defined boundary. Bounded runtime work and small initial process state matter as much as the queue algorithm.

BEAM's reductions are part of generated-code/runtime cooperation. BeamAsm emits native instructions while retaining that contract. Its scheduler is coupled to ERTS process state, execution conventions and memory management. Copying a run queue or reduction counter does not instrument a Rust loop. The NIF escape hatch itself has limits: long native calls require special handling and are not magically interruptible. [BEAM/Go source analysis](research/02-native-mechanisms.md#what-beam-actually-gives-us).

Do not promise BEAM's process-local garbage-collected heap behavior, cheap arbitrary process destruction, or Erlang term representation as a consequence of native green threading. Rust can retain typed owned state without a VM, but fixed native stack reservations and shared native libraries have different costs and failure behavior.

Literal source reuse is possible only with provenance and license conditions preserved. OTP's top-level license is Apache-2.0; individual source origins still matter. The initial recommendation is semantic reuse and test adaptation, not porting ERTS scheduler files wholesale. [Pinned OTP license](https://github.com/erlang/otp/blob/OTP-29.1.1/LICENSE.txt).

## Native implementation options

| Route | Fit for the request | Primary tradeoff |
|---|---|---|
| Signal-driven native fibers | Strongest immediate library-only candidate; locally demonstrated mechanism | Same-thread reentrancy, allocator/TLS integration, syscall behavior, signal ownership and stack safety. |
| Kernel-thread handoff | Native preemption with preserved interrupted native-thread state | Spare thread pressure; worst case can approach one kernel thread per suspended process. |
| Cargo-generated native process state machines | Mandatory checkpoints without handwritten yields or custom rustc; normal destruction at supported boundaries | Restricted, checked operation/type surface and complete transitive coverage; runtime-controlled cleanup. |
| Whole-program native compiler instrumentation | Broader automatic coverage may be possible | Separate compiler/toolchain contract; not the selected ordinary-Cargo experience. |
| OS thread per process | Native preemption baseline | Does not establish the intended tiny M:N abstraction. |
| Tokio/May/Glommio cooperative execution | Useful comparison and integration | Does not meet the defining no-manual-yields CPU-loop requirement. |
| Wasm/Lunatic/Wasmtime | Strong comparator from the lab | Different execution boundary than the accepted native product. |

Open-coroutine is the most immediately testable native candidate here. rrkernel supplies additional Linux fiber prior art, but its Windows backend uses one OS thread per task and its desktop embedding has materially different constraints. The libinger/libturquoise research demonstrates native preemptible functions but carries old platform/linker dependencies. None should be described as a turnkey validated foundation for Bream. [Candidate audit](research/03-runtime-landscape.md), [preemptible-function research](research/04-preemptible-functions.md), [mechanism alternatives](research/02-native-mechanisms.md).

For a stackful signal backend, a recommended initial policy is fixed worker affinity once a process starts. This avoids moving arbitrary live non-`Send` stack locals between OS threads, though it does not solve TLS reentrancy on the same thread or safe kill. The generated state-machine route has a different representation and should use normal Rust transfer rules rather than inherit native stack migration assumptions.

## Safe forced termination changes the leading architecture

The source audit of actual cancellation paths found no ready-made solution for the full contract. Open-coroutine's inspected running-task cancellation path suspends and then resets/reclaims a native stack without ordinary unwinding; the documented preconditions of that operation do not cover arbitrary live Rust state. May's cancellation depends on a later scheduling check. Corosensei can unwind supported explicit suspension boundaries, not arbitrary interrupted instructions. [Pinned cancellation audit](research/06-cancellation-candidates.md).

An arbitrary Rust frame cannot simply be discarded under the current language runtime assumptions. A zero-capture `'static` function can create an internal scoped child thread borrowing its stack; freeing the stack would invalidate that child's storage. Runtime-aware I/O and message-only intent do not prevent this by themselves. [Termination analysis and primary Rust sources](research/05-forced-termination.md).

The accepted next research route is a **Cargo-generated native process model**. A process attribute can insert unavoidable checkpoints, let standard rustc build a resumable state machine, and let the runtime stop polling and destroy supported state only after a checkpoint returned control. To meet the chosen BEAM-style cleanup contract, the admitted types and operations must exclude arbitrary user destructors and untracked escaping resources. A generic `Future` or `FnOnce` constructor would bypass that restriction.

This keeps native code, standard Cargo, and no author-written yields. It introduces a real tradeoff: a constrained, checked process programming model. A macro that accepts arbitrary Rust and adds checks to visible loops is not sufficient. The user authorized investigating that model; its broad usefulness and enforcement remain research gates. [Generated design](generated-process-model.md).

The distinction must stay public: signals interrupt native instructions asynchronously; generated processes suspend at mandatory checkpoints inserted by the library. Both can prevent an admitted application loop from monopolizing a worker without handwritten yields, but only the checked generated route currently offers a credible ordinary-Rust cleanup boundary for the stronger product contract.

The [generated-process experiment](../experiments/generated-process/README.md) tests that narrower mechanism directly: macro-admitted process source is compiled to native state machines, every loop head supplies an unavoidable suspension point, the single-worker harness drops the infinite process only after a poll returns, and tracked runtime-owned buffer cleanup completes while its peer progresses. Compile-fail fixtures exercise rejected escape paths. Read the final experiment report for exact results and exclusions; these are not arbitrary-code or production-runtime guarantees.

Final generated-probe observations:

- The infinite process reached 20 iterations over 21 polls, then was destroyed after `Pending`; it did not return voluntarily.
- Its tracked 4,096-byte buffer was dropped exactly once outside an active poll, with live payload accounting returning to zero.
- The surviving process progressed from 14 at the kill to its final value of 64.
- A separate bare `loop {}` with no builtins also returned `Pending` twice and was dropped outside polling.
- Ten unsupported-code fixtures were rejected, covering arbitrary calls, custom types/destructors, globals, macros, closures, unsafe code, manual await, reserved identifiers and raw identifiers.
- The two main future values occupied 72 and 64 bytes in this optimized build. Those figures exclude payload buffers, metrics, allocation headers and executor state; they are representation sizes, not full-process density.

[Final result and input hashes](../evidence/generated-process/bream-final/result.json), [verification](../evidence/generated-process/bream-final/verification.json).

## A useful library rather than an application framework

Make the independently useful primitive native process execution: spawn, continuation/stack ownership, automatic scheduling, wait/wake, join, exit observation and inspectable execution state. Supply runtime-aware channels and synchronization so long-lived code can block naturally without occupying its worker. Applications may transfer native typed values; forcing serialization would be an unrelated restriction.

Keep supervisors, actor behaviors, durable identities, cluster membership and hot upgrade protocols above the core. An optional Future adapter lets an existing Tokio application await Bream computation without owning the native scheduler. The existence of an adapter must not imply that an arbitrary Tokio task itself became preemptible.

“Generic” should mean useful across independent applications and higher layers. It should not mean an execution trait whose implementors are merely asked to yield promptly: that would weaken the very guarantee the library exists to provide. [Proposed boundary and API](native-design.md).

For the generated route, much of the distinctive work is the **checked program representation, automatic checkpoints, ownership and termination contract**, rather than a novel run-queue algorithm. An existing executor could schedule these enforced-yield state machines; it cannot confer the same property on arbitrary native futures. The local proof uses a tiny executor without Tokio to demonstrate independence. A later implementation may reuse an executor while keeping process semantics and the author-facing API independently useful.

## What would change the recommendation?

If a generated model cannot enforce safe resource ownership and termination while supporting useful stateful programs, the full accepted product remains unresolved. Signal switching or kernel-thread handoff alone cannot repair that failure. Broader compiler/runtime support would be a separately negotiated alternative. If native process density offers no useful benefit for the intended workload, revisit the implementation before adding actor features.

Conversely, successful safety qualification, useful density under realistic stacks/state, and composition with an ordinary host application would justify committing to a native foundation. Prefer contributing to an existing engine when it passes; creating a new public name is not itself a reason to reimplement its internals. [Qualification gates](qualification-plan.md).

## Naming

**Bream** is the user-selected working name: BEAM with an R for Rust, with a quiet fish joke. The project, probe crates and proposed APIs now use Bream. No package or remote repository has been published. [Naming decision and checks](naming.md).

## Latest source continuation — 2026-10-08

The [new reports 50–53](process-runtime-inventory.md) add system-task/completion ownership, code/literal retirement and typed registry routing. The plan now contains 523 stable cases and 99 performance experiments. New execution covers eight local naming families on three pinned VM profiles and narrow Rust variance controls; no performance treatment or production runtime was executed.

Concrete Rust candidates are typed owned maintenance requests, resumable reply settlement, permanent-static immutable bytes and carefully validated name/protocol routing. The source already coalesces some GC requests and hashes names by atom identity, so those costs cannot be claimed as new Rust wins without an equivalent baseline. Useful generated admission, preemptible helpers and safe forced stop remain the decisive implementation gate.

## Measured alternatives and the responsiveness priority — 2026-10-08

The [worker/async source pass](process-runtime-inventory.md) shows mechanisms Bream may simplify but cannot merely erase: disabled application dispatch can leave runtime service obligations, native job completion can outlive its recipient, and OS-process exit can conceal cleanup an embedded library must finish.

The user selected responsiveness first. All speed-test results and derived gains/regressions were subsequently removed at their request. The static-payload and worker-notification alternatives need fresh performance qualification. Their source and finite correctness models remain useful. The plan contains 537 cases and 107 tasks. [Removal record](../evidence/performance-reset/README.md).
