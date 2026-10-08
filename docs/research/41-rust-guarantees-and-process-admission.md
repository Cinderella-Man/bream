# Rust guarantees, process admission and executed ownership probes

Investigation: 2026-10-07. This report combines official Rust documentation with **12 isolated language probes compiled in two profiles: 24 compiler invocations and 12 executions**. All expected observations occurred using the retained Rust 1.98.1 toolchain. These are language/representation observations, not Bream process tests, automatic-preemption qualification or speed measurements. [Manifest and raw results](../../evidence/rust-ownership-review/probe-results.json), [runner](../../evidence/rust-ownership-review/run_probes.py), [reproduction and limits](../../evidence/rust-ownership-review/README.md).

The useful comparison is between the mechanisms BEAM needs for its dynamic term model and the work Bream might avoid with native typed state. [Heap roots](38-process-heap-roots-and-rust-ownership.md), [spawn publication](39-process-table-and-spawn-publication.md) and [allocator ownership](40-allocator-ownership-and-rust-runtime-storage.md) provide the corresponding source branches. They do not establish that changing implementation language makes an unchanged algorithm faster.

## Four different kinds of guarantee

| Layer | What it can establish | What remains outside it |
|---|---|---|
| Rust type and ownership checking | A consumed value cannot subsequently be used through its moved binding; incompatible typed protocols fail to type-check; borrowed data cannot escape the declared lifetime. | Scheduling progress, absence of globals, a reclamation deadline or absence of shared mutable state. |
| A closed Bream construction API | Only admitted state/program representations can be constructed through the supported safe entry points, if privacy, generation and every helper uphold that boundary. | A public marker or trait bound cannot establish properties its constructors and implementors do not enforce. |
| Runtime ownership/state protocol | One executor, publication/death ordering, identity validation, queued payload ownership and cleanup completion. | The borrow checker does not select the winner of concurrent kill/send/claim events or make a raw-pointer queue correct. |
| Internal unsafe and platform boundary | Audited atomics, allocation, pin projection, I/O registration and synchronization can support a safe surface. | `unsafe` code, allocator calls and platform behavior still need explicit invariants and evidence. Safe callers do not prove an unsafe implementation sound. |

This separates compiler facts from a proposed API and an implementation argument. Rust's [ownership rules](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html) and [privacy rules](https://doc.rust-lang.org/reference/visibility-and-privacy.html) can support the first two layers. `Send` means transfer between threads is safe, and `Sync` concerns shared references; neither names the process-lifetime contract. [Send](https://doc.rust-lang.org/std/marker/trait.Send.html), [Sync](https://doc.rust-lang.org/std/marker/trait.Sync.html).

## What the compiler probes actually establish

Each row was compiled with `opt-level=0` and `opt-level=3`, edition 2024. Fixtures forbid application `unsafe` code. The compiler is pinned in the result; live official documentation inspected this date showed standard-library version 1.99.0 and is supporting language guidance, not a claim that a 1.99.0 compiler executed these probes.

| Probe | Observed in both profiles | Design consequence |
|---|---|---|
| `moved_value` | Rejected with E0382. | Consuming an owning message can rule out sender reuse through that binding without runtime alias tracking. |
| `borrow_escape` | Rejected with E0597. | A declared lifetime boundary can block stack-borrow escape; construction must actually impose that boundary. |
| `rc_cross_worker` | Rejected with E0277. | Thread-transfer checks catch a non-thread-safe reference-counted value; this is only one dimension of admission. |
| `wrong_protocol` | Rejected with E0308. | A generic typed-address sketch rejects the wrong message enum. It contains no registry or mailbox and does not test identity/death semantics. |
| `pinned_move` | Rejected with E0277. | The safe `Pin::into_inner` route cannot extract this explicitly `!Unpin` value. |
| `owned_transfer` | Accepted/executed; four bytes preserved and the `Vec` backing pointer unchanged. | A simple owned handoff can preserve the backing allocation. No queue, allocation-count profile or end-to-end zero-copy guarantee was tested. |
| `send_sync_shared_mutation` | Accepted/executed; mutation through one `Arc<Mutex<_>>` observed through another. | `Send + Sync + 'static` admits shared mutable state that Bream's selected process contract excludes. |
| `forget_cleanup` | Accepted/executed; the instrumented destructor count remains zero. | Cleanup cannot depend on every application-owned handle eventually running `Drop`. |
| `manually_drop_cleanup` | Accepted/executed; the instrumented destructor count remains zero. | Looking only for a `forget` call would miss another safe way to suppress destruction. |
| `captureless_global` | Accepted/executed; a captureless callable updates shared global state. | Checking captured fields cannot establish transitive process ownership or execution effects. |
| `no_drop_callback` | Accepted; **compiled only**, never executed. | A `Copy` payload with no drop glue can carry a function pointer whose body does not return. Neither property makes invoking it admissible. |
| `pinned_borrow_cleanup` | Accepted/executed; one poll returns Pending and dropping the frame runs the bounded instrumentation destructor once. | Rust-generated suspended state can retain a local borrow and perform ordinary cleanup when dropped at a returned-poll boundary. |

The last fixture contains an explicit `await` solely to inspect Rust's generated storage. It is not proposed author syntax for Bream and does not establish automatic suspension. Its borrow points into `Vec` backing storage; no resumed read or self-referential frame layout was demonstrated. Similarly, the typed-address sketch and plain `Vec` transfer are independent language probes; none is a partial production API. Expected compiler rejection validates only these concrete fixtures, not every possible bypass.

## Ownership can remove work; it does not imply eventual cleanup

For an admitted owned payload, transfer can avoid reconstructing the dynamic object graph into a recipient's term heap. Native fields can also replace tag decoding and generic root traversal. The exact savings depend on the chosen representation: moving a large inline value may still copy its bytes, small-value boxing can introduce allocations, and shared buffers still require lifetime accounting. Compare room traffic and retained memory, not just a four-byte handoff. [Message-cost source trace](34-message-allocation-and-copy-costs.md), [Vec guarantees](https://doc.rust-lang.org/std/vec/struct.Vec.html#guarantees).

Rust permits safe code to leak resources. `mem::forget` and `ManuallyDrop` demonstrate why dropping the process frame is insufficient if admitted application code can intentionally or accidentally suppress the only release path. A useful runtime resource model needs a cleanup obligation that survives those operations, or an enforceable prohibition covering every admitted path. A plain trait bound cannot supply that policy. [Forget](https://doc.rust-lang.org/std/mem/fn.forget.html), [ManuallyDrop](https://doc.rust-lang.org/std/mem/struct.ManuallyDrop.html).

This does **not** license reclaiming memory that a forgotten handle may still expose elsewhere. The process model must establish which references can escape, transfer or remain in recipients, and how their obligations are retained. Process-local allocation regions, explicit registries and restricted owning values are alternatives to investigate. No one of them is selected here. A shared immutable allocation can outlive its publishing process by design.

`needs_drop::<T>()` answers a narrower question about destruction, with conservative true results permitted; it does not classify callable behavior, global access, internal allocation or reference escape. Our no-drop callback is an accepted counterexample to using that query as an admission certificate. [needs_drop](https://doc.rust-lang.org/std/mem/fn.needs_drop.html).

## Pinning lets the compiler retain borrows, with a storage obligation

Generated suspended computations can have address-sensitive state. `Pin` can support safe composition without manually rewriting every internal reference after movement, but the pointed-to storage must obey the pinning contract until the value is properly dropped. Moving a pointer owner is different from moving its pointee. Pinning does not guarantee a poll returns, prevent reentrant polling, permit destroying a running frame or establish a bound on destructor work. [Pin contract](https://doc.rust-lang.org/std/pin/index.html), [Future polling contract](https://doc.rust-lang.org/std/future/trait.Future.html).

The executed borrow fixture is a positive starting point for useful native state: rejecting every local borrow would discard a benefit Rust can provide. Before Bream admits such a body, generated checkpoints must preserve the valid pinned representation and one-poll ownership; external borrows and resource registrations require their own lifetime proof. Bulk arena deallocation underneath pinned values or live readers is not a valid shortcut. Rust also explicitly forbids stack-frame deallocation that violates the runtime's destructor assumptions. [Runtime assumptions](https://doc.rust-lang.org/reference/behavior-considered-undefined.html#undefined.runtime).

## The current macro is a research subset, not an effect checker

The existing checker accepts only a small set of scalar/buffer kinds, rejects external paths and rejects reading, moving or borrowing its owned buffer through ordinary user expressions. It emits an `async move` body and leaves the runtime helpers as ordinary public Rust types. That source is candid about its scope. [Checker](../../experiments/generated-process/macros/src/lib.rs:9), [buffer restriction](../../experiments/generated-process/macros/src/lib.rs:81), [generated body](../../experiments/generated-process/macros/src/lib.rs:353), [runtime boundary](../../experiments/generated-process/runtime/src/lib.rs:1).

Procedural macros consume and emit syntax/token streams. They do not receive a transitive proof that every resolved trait method, nested macro, callback or dependency returns and cleans up under the process contract. Generated code can enlist rustc to validate ownership/types, while Bream separately qualifies supported operations. Growing from the closed prototype to useful records, collections and helpers is still the decisive experiment. [Procedural-macro interface](https://doc.rust-lang.org/reference/procedural-macros.html).

## Two additional cases, plus refinements to existing admission work

The probes also inform existing LANG-01.C02–C07, LANG-02 and EXEC-03 obligations. They do not need twelve duplicate feature IDs merely because twelve fixtures ran.

| Case | Required treatment and evidence |
|---|---|
| LANG-02.C10 | Admitted code forgets an owning resource handle or retains it in ManuallyDrop; partial transfer/receiver death may overlap. Either enforced admission rejects the path or tracked cleanup obligations remain safely reclaimable without that destructor. Do not free a recipient-owned or escaped allocation to fix a leak. Rust probe acceptance is evidence of the language gap, not a Bream failure or passing Bream test. |
| LANG-03.C12 | A generated process holds local borrows across a mandatory boundary in address-sensitive saved state, then resumes, transfers an eligible pinned owner or is killed after poll return. Preserve pinned storage, borrow validity, exclusive execution and qualified cleanup. The isolated Pending/drop probe establishes only its safe compiler-generated example; active destruction and arbitrary user Drop remain excluded. |

## Next experiments

1. Build the useful typed-state slice with a consuming send and compiler-checked borrowed views. Pair each accepted example with an external consuming-crate bypass fixture; compare ownership walks and message allocation with the full-copy baseline.
2. Prototype two cleanup models for the same admitted handles: destruction-dependent ownership with all bypasses rejected, and independently tracked obligations. Include `forget`, `ManuallyDrop`, partial initialization and shared-byte transfer. Prefer evidence of complete accounting over a superficial marker trait.
3. Add borrowed-state/checkpoint/cancel fixtures to the generated model only after the public admission boundary exists. Check release profiles and pin projection; retain automatic checks in application bodies.
4. Compare specialized typed operations against an equivalent generic representation, including emitted code size and instruction-cache effects. Static dispatch is a mechanism to test, not a measured speedup.

The [Rust opportunity map](../rust-opportunities.md) distinguishes compiler guarantees from runtime obligations; the [performance plan](../performance-plan.md) gives experiments stable task IDs. Clear BEAM behavior remains the default. These results expose engineering obligations and do not require the user to choose different process semantics.
