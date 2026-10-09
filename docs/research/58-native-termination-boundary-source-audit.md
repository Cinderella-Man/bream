# Native termination boundaries: source audit of the proposed claim

Research date: **2026-10-09**. This is a source-only audit; no builds, experiments, speed tests, latency measurements or performance comparisons were run for this document. Rust library documentation inspected today reports **1.99.0**; that is documentation provenance, not the toolchain used by earlier experiments. BEAM sources are the local pinned **OTP-29.1.1**, commit **`ad05823719d77c8faee87348ea39513d4e2f99c5`**. Existing context: [forced termination](05-forced-termination.md), [BEAM scheduling](11-beam-scheduling-source.md), and [Rust admission probes](41-rust-guarantees-and-process-admission.md).

The proposition under review:

> I went looking for the hard part of building BEAM in Rust. Preemption wasn't it—we got native code taking turns. The hard part is killing one process without breaking another. Rust's ownership rules don't automatically give you that boundary.

## Verdict

**The ownership-boundary claim is supported. The claim that preemption is already solved, or is generally easier, is not.** Taking turns in the earlier restricted arithmetic program proves a scheduling mechanism exists. It does not qualify that mechanism for arbitrary libraries, allocation, locks, thread-local state, foreign calls or interrupted cleanup. Termination requires additional ownership, execution and resource-lifetime contracts; Rust's normal safe APIs do not supply them automatically. This is a gap between memory safety and a particular process contract, not a Rust memory-safety defect or an impossibility result.

| Claim | Assessment | Evidence or limitation |
|---|---|---|
| Native Rust computation can be preempted without author-written yields. | Supported for the retained arithmetic fixture. | [Probe scope and results](../../experiments/native-preemption/README.md), including the negative control. |
| Any dependency is now safe to preempt, migrate and resume on a worker. | Unproven. | The same probe explicitly excludes allocation, lock, TLS, FFI, aliasing, migration and signal-composition qualification. |
| Safe Rust ownership automatically makes a task a disposable isolation domain. | Refuted. | `Send`/`Sync` permit shared state; native child threads, arbitrary `Drop`, resource leaks and scoped borrowing have separate contracts. See below. |
| A suspended future can be dropped normally at a returned-poll boundary. | Supported subject to the future's normal Rust contract. | This is the ordinary async cancellation route; it does not imply rollback, bounded destruction or termination of independently spawned work. |
| A frozen arbitrary Rust stack can just be freed or unwound from any instruction. | Unsupported and incompatible with documented obligations in concrete cases. | Stack-frame runtime assumptions, pinning, scoped threads, allocator non-unwinding and library exception-safety reasoning all matter. |
| BEAM process exit rolls back everything the process did or immediately releases every resource. | Refuted. | Erlang exit, directly visible resources, heap reclamation and dirty NIF completion are explicitly different phases. ETS and messages also expose completed effects. |
| These facts prove Bream is a bad idea or necessarily needs a tiny crate allowlist. | Unproven. | A controlled ownership model, generated boundaries, qualified operations or stronger isolation can change the available contract. The useful extent remains a research question. |

## Four observations that must remain separate

1. **Pause/resume:** preserve a continuation so it can run again. This does not establish that everything it references is private, that another actor can safely use the same worker state, or that the paused frame is disposable.
2. **Logical stop:** the runtime will not resume the actor and rejects further supported actor operations. Independently started native threads and external operations are not automatically covered.
3. **Cleanup:** release or transfer obligations while preserving surviving references and registrations. Retaining a frozen stack is quarantine, not complete reclamation.
4. **Peer integrity:** other actors retain valid memory and usable runtime resources. This must not be confused with application-level rollback: already committed messages or external writes may remain observable.

These are analytical definitions for the experiments. BEAM's documented distinction between exiting state, released visible resources, delayed heap reclamation and continuing dirty native work provides a concrete runtime precedent. [Pinned process manual, lines 305–310](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L305-L310), [lines 462–493](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L462-L493).

## What ordinary Rust does and does not promise

### Transfer and sharing are not containment

`Send` governs thread transfer; `Sync` governs sharing references between threads. An `Arc<Mutex<T>>` can satisfy both while providing shared mutable state. Those bounds do not state that all data reached by a task belongs exclusively to that task. The `'static` spawn bound prevents the spawned closure from carrying shorter-lived borrows; it neither forbids globals nor prevents the body from creating new threads and shared handles. [Send](https://doc.rust-lang.org/std/marker/trait.Send.html), [Sync](https://doc.rust-lang.org/std/marker/trait.Sync.html), [thread::spawn](https://doc.rust-lang.org/std/thread/fn.spawn.html).

A peer can therefore observe a partially completed application protocol after cancellation while the program remains memory-safe. Mutex poisoning is advisory and panic-related; it is not rollback, and ordinary future destruction is not automatically a panic. `UnwindSafe` likewise warns about logical invariant exposure, rather than certifying arbitrary cancellation. [Mutex poisoning](https://doc.rust-lang.org/std/sync/struct.Mutex.html#poisoning), [UnwindSafe](https://doc.rust-lang.org/std/panic/trait.UnwindSafe.html).

**Experimental implication:** a safe `Send + 'static` future may update phase one of shared state, return `Pending`, and be destroyed before phase two. A peer can then read the incomplete protocol through a normally acquired mutex. Keep the guard out of the suspended frame if the test claims `Send`: standard `MutexGuard` is not `Send`. This demonstrates missing application isolation, not memory corruption and not a uniquely Rust problem. [MutexGuard](https://doc.rust-lang.org/std/sync/struct.MutexGuard.html).

### Stopping the parent does not automatically stop native children

Dropping a native `JoinHandle` detaches its thread; the thread may continue independently. A future can spawn such a child, suspend, and later be dropped while the child keeps working. This is intended Rust behavior. A process runtime must track, reject or separately contain that work if its advertised stop contract includes it. [JoinHandle](https://doc.rust-lang.org/std/thread/struct.JoinHandle.html).

Scoped threads add a different obligation: they may borrow local data because `thread::scope` joins them before returning. A captureless, `'static` process entry can create local state and subsequently lend it to a scoped child. Reclaiming a frozen parent's stack or local allocations while that child still reads them bypasses the scope contract. Retaining the backing storage avoids this particular reclamation error but does not terminate the child. [Scoped threads](https://doc.rust-lang.org/std/thread/fn.scope.html).

**Experimental implication:** use channel-controlled child work and explicit joins. Prove that a child produces an acknowledged effect after parent-future destruction, then shut it down cleanly. For scoped borrowing, demonstrate the ordinary join/lifetime obligation without actually freeing live borrowed storage. Do not introduce undefined behavior to make a dramatic counterexample.

### Running destructors is not bounded cleanup; skipping them is not generic reclamation

`Drop::drop` is ordinary synchronous user code. Its signature provides no eventual-return guarantee. A destructor can wait for another thread, invoke library code or panic. A panic during an existing unwind can abort the host. Tokio consequently includes destructors in its warning about blocking operations on executor threads. [Drop](https://doc.rust-lang.org/std/ops/trait.Drop.html#tymethod.drop), [Tokio blocking and cancellation](https://docs.rs/tokio/latest/tokio/task/index.html#blocking-and-yielding).

Conversely, safe Rust permits `mem::forget`: destructor execution is not universally guaranteed. That permits leaks, not arbitrary invalidation of storage still used by survivors. Pinning explicitly requires maintaining the pinned value's storage until its destruction obligation is met; permanently retaining storage differs from freeing it. The Rust Reference also documents the stack-frame assumption against removing frames without executing owned-local destructors. [Forget](https://doc.rust-lang.org/std/mem/fn.forget.html), [Pin drop guarantee](https://doc.rust-lang.org/std/pin/index.html#subtle-details-and-the-drop-guarantee), [runtime assumptions](https://doc.rust-lang.org/reference/behavior-considered-undefined.html#undefined.runtime).

**Experimental implication:** a destructor controlled by a channel is sufficient to prove cleanup depends on outside cooperation. Have it acknowledge entry, verify cleanup has not completed before permission is sent, then release and join it. No timing comparison or deliberately infinite destructor is necessary. A separate drop-counter fixture can demonstrate forgotten resources without pretending a leak is memory unsafety.

### Arbitrary instruction interruption is not an ordinary panic boundary

The Rustonomicon's exception-safety reasoning explicitly permits internal operations whose proof relies on a region not invoking panicking code. Injecting unwinding at a new arbitrary instruction can invalidate that reasoning. Global allocators have an explicit non-unwinding requirement. Neither a catchable panic payload nor `UnwindSafe` establishes an asynchronous cancellation contract for every native frame. [Exception safety](https://doc.rust-lang.org/nomicon/exception-safety.html), [GlobalAlloc safety](https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html#safety).

This does **not** say saving registers and later resuming is impossible. It says resumption and destruction need distinct proofs. A safer experimental route is ordinary future destruction after its `poll` returned, using exclusive ownership of that suspended future. Rust documents that `poll` should return quickly; it does not equip an ordinary executor to forcibly return from an arbitrary synchronous call within it. [Future contract](https://doc.rust-lang.org/std/future/trait.Future.html#runtime-characteristics).

## What BEAM contributes, and the limit of the comparison

BEAM establishes scheduling boundaries in the execution representation. Its interpreter decrements `FCALLS` and enters a context switch when the allowance is exhausted; the x86 JIT emits a decrement and branch to its yield fragment. Thus native machine code and automatic scheduling coexist when the compiler and runtime cooperate. This is not a property supplied by the C/C++ implementation language. [Interpreter, lines 154–165](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/emu/macros.tab#L154-L165), [JIT, lines 3217–3241](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/instr_common.cpp#L3217-L3241).

Ordinary term messages get recipient-owned representations; the inspected send path allocates message storage, copies non-immediate terms as applicable, and queues the resulting message. Runtime-controlled representation and ownership make process reclamation a tractable runtime responsibility, rather than an inference from an arbitrary native closure's captures. Shared immutable binaries and other off-heap resources still have separate lifetimes. [Message send, lines 821–874](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.c#L821-L874), [final queue and off-heap cleanup, lines 13405–13447](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13405-L13447).

Exit handling includes explicitly resumable link and monitor teardown. That is a concrete example of lifecycle work designed around scheduler progress. It does not prove all destruction has a fixed elapsed-time bound: the final deletion path also includes queue/off-heap cleanup and linear loops. [Yielding teardown, lines 14545–14584](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14545-L14584), [final deletion, lines 13393–13447](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13393-L13447).

Native extensions cross this boundary. OTP warns that a NIF does not receive ordinary Erlang scheduling/memory protection and can compromise the VM. Dirty NIF termination releases directly visible Erlang resources and triggers relationship notifications, while native execution continues and certain storage remains retained. The dirty-call source invokes the native function, then checks the exiting state after control returns. Shared NIF resources and static data also require explicit synchronization. [NIF limits, lines 34–47](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L34-L47), [shared data, lines 271–287](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L271-L287), [dirty termination, lines 477–489](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L477-L489), [native call and return, lines 468–507](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L468-L507).

**Do not claim BEAM rolls back effects.** ETS guarantees atomicity/isolation for specified individual operations, not an arbitrary sequence of application operations. A process can update a table owned by a surviving process and die before a later update; a previously sent message also remains a committed effect. The fair comparison is containment of execution and resource lifetime, not universal transactional behavior. [ETS operation scope, lines 126–136](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/stdlib/src/ets.erl#L126-L136), [table access, lines 1244–1256](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/stdlib/src/ets.erl#L1244-L1256), [message publication above](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.c#L869-L874).

## Implications for the bounded correctness experiment

The following cases can separate language facts from unproved runtime claims without benchmarking:

| Case | Deterministic observation | What it establishes |
|---|---|---|
| Owned local state with qualified destruction | Poll once, cancel at `Pending`, exact drop receipts, independent peer still completes. | Positive control: useful cancellation is possible under a controlled contract. |
| Shared protocol interrupted between stages | Peer reads the acknowledged first stage after parent cancellation. | Safe sharing is not transaction rollback; include the BEAM ETS/message qualification. |
| Native child survives parent-future drop | Explicitly release child work only after parent destruction, receive its acknowledgement, then join. | Nominal parent ownership does not automatically contain spawned execution. |
| Cleanup requires cooperation | Destructor announces entry, then completes only after a controlled release signal. | Logical stop and complete cleanup differ; no latency inference needed. |
| Destructor suppressed | Observed drop count remains zero after an admitted safe forgetting operation. | Eventual cleanup requires a stronger resource model or enforced restrictions. |
| Scoped borrow outlives parent's scheduling decision | Child uses borrowed local storage, parent retains it until scoped join completes. | Stack reclamation must preserve existing lifetime contracts; avoid an unsound forced-free test. |

These cases would refute a general promise based only on ordinary Rust ownership and `Send + 'static`. They would **not** refute a sealed checked Bream model that excludes these operations, prove every dependency needs an allowlist, or validate general signal-driven cancellation. No new experimental outcomes are asserted by this source audit.

## Wording supported by the evidence

“We got two native Rust computations taking turns. That did not make either one safely disposable. Rust lets a task share state, start native threads and run arbitrary destructors; ownership alone does not turn that task into a BEAM-style process.”

For a shorter account, retain the distinction between **we demonstrated native scheduling** and **we have not qualified arbitrary native process death**. Avoid “preemption wasn't the hard part” as an objective conclusion until broader preemption composition has been tested, and avoid “without breaking another” unless the relevant guarantee—memory safety, lock availability, continued execution, resource lifetime or application invariant—is named.
