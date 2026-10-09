# What Rust's old green runtime teaches Bream

**2026-10-09. Historical source research plus original-runtime and modern correctness experiments. No speed tests or performance comparisons.**

The important discovery is that **Rust had green threads without having solved all the concerns in this project**. Its 2014 runtime was cooperative, openly documented native-library limitations, and no longer included the earlier linked-task cancellation system. That cancellation system was removed in 2013 after concrete race and I/O failure-safety concerns. This history supplies useful mechanisms and failure cases; it does not prove that a modern Rust process runtime is impossible.

Research details: [removal history](60-rust-green-runtime-removal-history.md), [scheduler and lifetime source audit](61-rust-green-runtime-source-contracts.md), [2013 linked-failure audit](62-rust-linked-failure-source-contracts.md). Executable details: [method and limitations](../../experiments/rust-green-history/README.md), [passing cohort](../../evidence/rust-green-history/runs/20261009-green-02/results.json), [artifact audit](../../evidence/rust-green-history/validation.json).

## The PRs answer different questions

| Record | What it establishes |
|---|---|
| [PR #10603, merged 2013-11-25](https://github.com/rust-lang/rust/pull/10603) | Removes linked task failure. Reasons include model/implementation complexity, known scheduler context-switch races, and uncertainty about native-thread semantics. It leaves possible conservative reimplementation open. |
| [RFC #230, accepted 2014-09-16](https://rust-lang.github.io/rfcs/0230-remove-runtime.html) | Proposes removing the standard library's combined native/green runtime abstraction and moving green support to a separate package. Native and green I/O requirements had become difficult to serve through one interface. |
| [PR #18967, merged 2014-11-21](https://github.com/rust-lang/rust/pull/18967) | Removes the remaining Runtime trait, libnative and libgreen. Other I/O and runtime-crate removals happened in separate changes. |

The late 2014 source explicitly states that tasks have no preemption; an infinite loop prevents other tasks on that scheduler from running. It warns about native blocking, native TLS, OS-thread synchronization and fork. Its ordinary context switch also relies on normal call-ABI register obligations. A context-switch routine is not a complete arbitrary-instruction interruption mechanism. [Scheduling and interoperability contract](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/lib.rs#L51-L111), [register contract](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/context.rs#L132-L146).

The earlier cancellation code is even more relevant. A kill request sets pending state and can wake a blocked task. The task delivers that failure by unwinding at runtime boundaries, unless killing is masked. It does not asynchronously unwind arbitrary instructions. File I/O masks cancellation while callbacks can still refer to local request state. Stream code contains unresolved cancellation comments and guards against unwinding through an outstanding operation. [Deferred delivery](https://github.com/rust-lang/rust/blob/85a1eff3a99c5d60bf34ca6f08f87ed341329af0/src/libstd/rt/kill.rs#L630-L671), [filesystem operation](https://github.com/rust-lang/rust/blob/85a1eff3a99c5d60bf34ca6f08f87ed341329af0/src/librustuv/file.rs#L287-L329), [stream operation](https://github.com/rust-lang/rust/blob/85a1eff3a99c5d60bf34ca6f08f87ed341329af0/src/librustuv/stream.rs#L67-L132).

[Issue #8674](https://github.com/rust-lang/rust/issues/8674) directly reports failed recovery when linked failure resumes a blocked I/O task. The proposed immediate response was to defer cancellation during I/O and qualify interruption operation by operation. This is stronger evidence than a retrospective claim that destructors might be troublesome. See [report 62](62-rust-linked-failure-source-contracts.md) for the guard/double-unwind path and its limits; no historical crash was reproduced here.

## We executed the original runtime

The official Rust 0.12 archive still runs on this host. Its checksum matches the release team's [published value](https://internals.rust-lang.org/t/0-12-0-release-candidates/618), and its compiler revision resolves to `ba4081a5a8573875fed17545846f6f6902c8ba8d`. It was unpacked into ignored workspace storage; no global installation was made.

Four fixtures ran on **one green scheduler thread**, with the native host orchestrating them. Each passed with default optimization and historical `-O`:

| Original-runtime observation | Interpretation |
|---|---|
| A's atomic no-yield gate finishes before queued B executes. | Matches documented cooperative behavior. An external host releases the gate; no endless task or elapsed-time threshold. |
| With explicit yields, B executes while A remains in its active phase and releases A. | Real green-task switching and continuation through ordinary Rust code. The local sentinel is not exhaustive machine-context qualification. |
| A green channel wait lets a child run; parent task-local value 11 and child value 22 remain separate. | Task-aware waiting and task-owned local context are real mechanisms. Native TLS migration was not tested. |
| Intentional task panic returns an error, drops a guard once, and leaves an independent task able to return 17. | Ordinary panic containment and cleanup work for this fixture. This does not execute the removed linked-kill mechanism. |

These are event/ownership observations, not scheduling-rate or latency results. The 2013 source, October 2014 executable and November 2014 removal snapshot remain explicitly separate baselines.

## Two mechanisms worth carrying forward

### 1. Let an operation own storage beyond the actor's lifetime

The dangerous historical shape is an external callback retaining a pointer into a task that cancellation wants to unwind. An alternative is to transfer the request buffer into a runtime operation record. Actor execution can end while that record stays alive until completion or acknowledged cancellation. An eventual result is delivered to the original identity or disposed of.

A modern safe-Rust prototype checks both completion orders. On normal completion the actor gets the buffer/result. When its frame and receiver are dropped first, the driver still owns the buffer; another application step runs before the host releases the callback; failed delivery then releases the buffer exactly once. A separate routing case rejects a completion for generation 1 after its slot names generation 2. [Prototype](../../experiments/rust-green-history/modern.rs), [exact results](../../evidence/rust-green-history/runs/20261009-green-02/results.json).

A compiler rejection control also confirms that safe Rust prevents dropping a buffer still borrowed by a scoped callback. That is the constructive lesson: **ownership can enforce the boundary once the operation is represented with the right owner.** It does not automatically cancel the operation or restore application invariants.

This prototype uses a gated driver, not real native I/O. The independent application step is not Bream scheduler integration. If the operation never completes, storage can remain retained indefinitely. A usable adapter still needs resource limits, admission/backpressure, failure handling and shutdown semantics. Moving ownership also does not undo an external write or allow safely killing arbitrary foreign code.

### 2. Treat wake ownership and callback completion separately

The old scheduler could publish a blocked task while its registration callback still borrowed that task's stack. Another scheduler could win the wake and resume the task before the callback returned. A per-task lock delayed resumption until the callback stopped accessing the borrowed environment. [Historical barrier](https://github.com/rust-lang/rust/blob/770378a313a573776b16237a46b75bafa49072c1/src/libgreen/sched.rs#L718-L779).

The [original issue #8132](https://github.com/rust-lang/rust/issues/8132) even proposed a consuming environment-permission token. The modern prototype explores both approaches. Across the three abstract orders with wake preceding attempted resume, one naive order resumes while registration remains active. A completion barrier avoids that overlap. An ownership sketch consumes one registration wrapper before producing an opaque token; the compiler rejects reuse of that wrapper after consumption. The token is not tied to a particular task or registration, and no real wake API consumes or validates it. A token made from an unrelated environment would have the same type. Enforcing actual publication therefore still needs identity binding and a controlled wake API.

This is a small ordering model and type-boundary control, not a memory-model proof or a full scheduler. Its value is a precise review rule: **winning the right to wake does not establish that every borrower has finished.** Avoid recreating the old lifetime-erased callback interface when modern ownership can express the handoff directly.

## Implications for the existing Bream implementation

Our current restricted single-worker runtime already retires a frame only after polling returns, distinguishes terminal identity from later cleanup, and keys timer callbacks by exact process/registration identity. Timer callbacks use retained/weak metadata rather than pointers into an application frame. The historical findings do not demonstrate a bug in those paths. [Poll/retirement order](../../experiments/session-replacement/runtime/src/lib.rs), [terminal and cleanup phases](../../experiments/session-replacement/runtime/src/lifecycle.rs), [timer ownership and stale checks](../../experiments/session-replacement/runtime/src/timer.rs).

They identify the next obligations **before admitting external I/O or multiple workers**:

1. Specify actor execution stopped, registration finished, native operation finished and storage released separately. Existing Down must not acquire an undocumented promise that every external effect or operation is finished.
2. Give each external operation an owning buffer/request record and full process identity. Test stop before submission, during registration, before/after completion, duplicate completion, identity reuse and driver shutdown.
3. Prefer consuming registration APIs. If a backend must borrow actor state, require an explicit completion/cancellation acknowledgment before reclaiming it. Do not hide indefinite cleanup in actor Drop.
4. Preserve task-owned context and qualify native thread-affinity/TLS assumptions separately. Pinning can address migration assumptions; it does not make native blocking cooperative.
5. Describe compatibility by operation: controlled computation, runtime-aware waits, borrowed native operations and independently owned/offloaded work require different contracts. A blanket whole-crate label would hide useful distinctions.

These are inferred design directions, not completed production features. The best next resource experiment is a bounded adapter using a real cancellable I/O backend, integrated with current stop, stale-identity and cleanup accounting. Its exit conditions should require exactly-once ownership disposal, no callback access to a reclaimed frame, no result delivered to a replacement process, and independently observable outstanding-resource cleanup. No backend or performance profile is selected by this research.

Potential improvements are structural hypotheses: owned transfer can avoid a defensive buffer copy; consuming registration may avoid a callback-lifetime lock; generated frames can retain only continuation state instead of an entire native stack. This work measures none of those costs, and a real implementation must establish its own allocation and synchronization behavior before any performance qualification elsewhere.

## Result and limits

The passing cohort contains **26 expected command outcomes and 20 observations**: eight original-runtime results, eight modern results, and four expected compiler rejections across two build profiles. The read-only artifact audit passes 434 checks. Seven exact experiment/runner/setup inputs, source manifests, tool identities and command outputs are retained. The failed first cohort is documented: it used an unsupported modern optimization flag with the old compiler and never ran a scenario.

The premise that all these concerns must have been covered because Rust shipped green threads is false. The more useful conclusion is that Rust's history already contains both this project's failure modes and plausible solutions. We should reuse the ownership and lifecycle lessons, while retaining Bream's narrower admission guarantees. Nothing here proves that preemption is fully solved, that arbitrary dependencies can be safely killed, or that Rust is intrinsically unsuitable compared with C/C++.

This is a completed research milestone, separate from the implementation count. The then-planned checked self-exit/local-exit work subsequently passed as [implementation milestone thirteen](65-explicit-exit-milestone.md). The investigation was then applied in [implementation milestone fourteen](67-owned-read-milestone.md): a bounded runtime-owned external read. The [implementation progress list](../milestones.md) tracks subsequent resource work.
