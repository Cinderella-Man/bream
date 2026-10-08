# Rust atomic protocol observations

**2026-10-07. Three small synchronization projections, seven isolated Loom invocations: three positive variants complete and four deliberate weakenings produce the expected invariant violations.** This is executed model evidence, separate from BEAM source inspection and ordinary hardware stress. It does not implement Bream, its preemption engine, process destruction or a complete reclamation algorithm.

The result sharpens a Rust opportunity: narrow ownership APIs can centralize a synchronization proof, allowing callers to avoid repeating complex checks. Safe Rust and atomic types do not establish that proof by themselves. Every model is compiled with `#![forbid(unsafe_code)]`; the negative variants still lose publication, cleanup visibility or a wakeup.

## Scope and reproducibility

The [source](../../experiments/atomic-protocol-models/src/main.rs), [locked dependency graph](../../experiments/atomic-protocol-models/Cargo.lock), [runner](../../evidence/concurrency-clock-review/run_models.py) and [results](../../evidence/concurrency-clock-review/model-results.json) are retained. Build: Rust 1.98.1, release profile, Loom 0.7.2. One build produced one executable; seven independent child invocations each explore a named variant. Raw assertion output is preserved, including the failing values.

Each model has two threads, finite straight-line protocol work and no spin loop. All shared state used in decisions uses Loom primitives. A standard atomic counter only counts starts of the model closure; it never influences an algorithm branch. The builder allows 1,000 branches per execution, with no configured preemption bound, permutation cap, internal duration cap or checkpoint resume. External watchdog expiry is failure. None expired and no branch limit fired.

Loom explores instrumented concurrency and uses reduction techniques; it does not cover code using ordinary synchronization behind its back. It also documents incomplete modeling of some memory reorderings. Consequently a completed run here means no modeled violation was found within this projection and these settings, not exhaustive Rust-memory-model correctness. Counts below are closure starts, not unique states, fairness results or source-coverage percentages. [Loom 0.7.2 and limitations](https://docs.rs/loom/0.7.2/loom/), [builder settings](https://docs.rs/loom/0.7.2/loom/model/struct.Builder.html).

| Projection / variant | Observed result | Executions started |
|---|---|---:|
| Publish payload, release flag, acquire flag before payload read | Completed | 6 |
| Same publication with relaxed flag store | Failed: `ready=true`, payload still 0 | 3 |
| Same publication with relaxed flag load | Failed: `ready=true`, payload still 0 | 3 |
| Two owners release count; last owner uses acquire fence before inspecting writes | Completed; one finalizer | 4 |
| Same final-owner protocol without acquire fence | Failed: observed writes `(1,0)` | 1 |
| Empty-check/wait publication and message/wake decision under one mutex | Completed | 4 |
| Separate release stores and acquire loads for message/wait flags | Failed: neither side saw the other's publication | 1 |

## Publication is a relation between operations

The writer stores payload 42 with relaxed ordering, then publishes a ready flag. The reader only inspects payload when it sees ready. With the release/acquire pair the tested projection completes. Weakening either side permits the recorded stale payload result. The payload itself is atomic, so this fixture exposes an ordering error without causing a Rust data race or dereferencing an invalid pointer.

The standard library documents that a matching acquire read can make prior writes visible through release publication. It does not make every acquire operation synchronize with every release on another atomic. Read-modify-write orderings also apply differently to their load and store portions. [Rust atomic ordering contract](https://doc.rust-lang.org/std/sync/atomic/enum.Ordering.html).

For Bream, the implication is a review obligation: state exactly which operation publishes a completed field set and which observation authorizes reading it. A PID, generation or refcount value alone is not that argument. Early process publication can expose only a valid partial state while other fields remain protected by locks; the source mapping in [report39](39-process-table-and-spawn-publication.md) must not be simplified into “one ready bit publishes everything.”

## Last owner is not automatically synchronized cleanup

The second model starts with two conceptual owners and two independently written fields. Each owner writes its field and decrements a shared count using release ordering. Whoever observes the transition from one to zero becomes the sole finalizer. With an acquire fence before inspecting both writes, the model completes. Removing the fence leaves the count transition intact but admits the recorded stale second field.

This is a visibility projection, **not an implementation or test of Rust `Arc`**, nor of BEAM reference counting. A separate Loom Arc keeps the model storage alive throughout; no raw storage is actually freed when the conceptual count reaches zero. The fixture establishes neither reference conservation with cloning nor resurrection, overflow, weak upgrades or allocator safety. Its narrow point is that winning final ownership and acquiring the prior owners' writes are separate obligations. The applicable fence synchronization rules require a specific atomic read/write relationship. [Rust fence contract](https://doc.rust-lang.org/std/sync/atomic/fn.fence.html).

Possible optimization: put the required synchronization in one qualified finalization path instead of over-synchronizing every unrelated read. That requires the actual ownership proof and comparison with already optimized primitives; this experiment measures no cost and does not authorize weakening existing atomics. The distinct metadata, body and callback lifetimes in [reports46](46-thread-progress-and-retirement-domains.md) and [44](44-native-resource-callback-and-owner-lifetime.md) still apply.

## Two ordered flags do not form a parking protocol

The locked model lets the receiver commit to waiting only while observing an empty slot under the same mutex used by the sender to publish a message and decide whether it owes a wakeup. After both finish, a committed waiter must have a wake obligation.

The negative variant instead lets the sender release-store message availability then acquire-load the waiting flag, while the receiver release-stores waiting then acquire-loads message availability. Loom finds both loads observing false. The receiver would wait, while the sender sees no reason to wake it. Merely choosing acquire/release orderings on both variables is insufficient.

This is a small counterexample to a proposed shortcut, not a port of BEAM's parking machinery. It has one message and one waiter, no actual sleep, no queue reservation, no spurious wake, no priority, no kill and no second publisher. The earlier [finite parking model](33-finite-ownership-interleavings.md) covers different schedule-out obligations under sequential consistency. The real BEAM [parking trace](31-receive-parking-wakeup-protocol.md) retains MSGQ through the final check; [report47](47-process-lock-ownership-and-handoff.md) adds lock acquisition and handoff details. None of these independently proves the whole scheduler.

## Qualification and next steps

The useful Rust design target is a small internal API whose guards distinguish valid record access, lock-protected field access and application execution permission. Process code must not retain runtime lock guards indefinitely across automatic suspension or termination. Borrow checking can constrain reference escape through such an API; runtime enrollment, lock progress, host misuse boundaries and final reclamation still need explicit contracts.

| Case | Required qualification |
|---|---|
| REF-02.C09 | A candidate weak-memory optimization changes publication, final-owner visibility or parking synchronization. Retain positive and deliberately weakened models, exact ordering/bounds/tool identity and counterexamples; distinguish model completion from Rust soundness, hardware fairness and Bream preemption. All protocol-visible shared operations must be modeled or explicitly excluded. |

This case refines the conformance harness, not a newly qualified runtime feature. Next apply the same discipline to a real chosen internal protocol, include pointer validity and ownership conservation, then combine model exploration with targeted executable stress. Separately qualify lock backends and clock conversion/provider branches from the new source reports. No production engine, epoch library, lock policy, clock policy or atomic weakening is selected here.
