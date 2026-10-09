# Rust green-runtime history: executable correctness probes

2026-10-09. [Conclusions](../../docs/research/63-rust-green-runtime-lessons-and-experiments.md), [historical source contracts](../../docs/research/61-rust-green-runtime-source-contracts.md), [earlier cancellation source](../../docs/research/62-rust-linked-failure-source-contracts.md), [evidence](../../evidence/rust-green-history/README.md).

These probes execute **the original Rust 0.12 green runtime**, then explore two mechanisms using modern safe Rust. They do not run benchmarks or collect durations. No production Bream runtime changed.

## Historical baseline

The compiler is the official Linux x86-64 Rust 0.12 archive, checked against the release team's published SHA-256. It reports `rustc 0.12.0 (ba4081a5a 2014-10-07 13:44:41 -0700)`. Full source revision: `ba4081a5a8573875fed17545846f6f6902c8ba8d`. It is unpacked only under ignored `.tools/rust-green-history`; no installer or global toolchain changes. [Archive provenance](../../evidence/rust-green-history/provenance.json), [official checksum announcement](https://internals.rust-lang.org/t/0-12-0-release-candidates/618).

`historical.rs` configures exactly one green scheduler thread and the basic event loop. Its native host remains outside that pool. It exercises channels, not historical libuv I/O. This compiler is **after** the 2013 linked-failure removal; no earlier task-kill behavior is claimed to have been executed.

| Case | Protocol and exact oracle |
|---|---|
| `no_yield_gate` | Task A enters an atomic gate with no scheduling calls. The native host queues B, then releases A. B observes A's finished phase. No intentionally endless loop or speed target. |
| `cooperative_yield` | A explicitly yields at its gate. B observes A's active phase and releases it. A verifies its local sentinel and completes. The sentinel is a fixture assertion, not exhaustive register/stack preservation qualification. |
| `channel_and_task_local` | A waits on a channel; its child must execute on the same scheduler to answer. Child task-local state starts empty and becomes 22; parent retains 11. |
| `panic_cleanup` | An intentional ordinary panic becomes an error result. The local guard drops exactly once; an independent task returns 17. No external kill, destructor panic, or foreign fault is exercised. |

Channel operations themselves can call `maybe_yield` in this runtime. None occur inside the no-yield gate. Host release bounds that scenario by protocol; an external watchdog covers broken execution. A passing finite trace supports the source contract but cannot prove the absence of every possible preemption mechanism by observation alone.

## Modern mechanism probes

`modern.rs` uses Rust 1.98.1 and `#![forbid(unsafe_code)]`. It is an independent protocol prototype, not Bream integration or a port of historical scheduler code.

| Case | Exact oracle |
|---|---|
| `owned_complete` | Driver owns the request buffer until explicit completion; live actor receives value 7; frame and buffer each drop once. |
| `owned_cancel` | Actor frame and receiver drop while driver is gated; buffer remains alive. A separate application step produces 17 before callback release. Delivery fails after release and the undelivered buffer drops once. |
| `reused_identity` | A late completion names slot 1/generation 1. Router now names generation 2, rejects it, retains replacement value 99, and drops the old buffer once. This models identity replacement; it does not invoke the existing Bream registry. |
| `wake_registration` | Enumerate all three abstract event orders with wake before attempted resume and callback completion in any position. Immediate resumption overlaps registration once; a callback-completion barrier avoids overlap in all three. A consuming registration token provides a separate API control. |
| `reject_borrowed_drop.rs` | Compiler rejects buffer destruction while a scoped callback still borrows it (`E0505`). |
| `reject_registration_reuse.rs` | Compiler rejects access through registration after `finish(self)` consumes it (`E0382`). The opaque token has no public constructor or borrowed environment, but it is not bound to a task/registration identity and no wake API validates it. |

The owned driver is a **channel-gated stand-in for pending native work**. It neither performs real I/O nor cancels an OS operation. Its host retains and joins the driver after explicit release. The application step before release establishes ordering, not Bream scheduler responsiveness. A permanently blocked operation could still retain the buffer indefinitely; quotas, acknowledged cancellation, driver failure and shutdown need additional work.

The wake model checks a small lifetime ordering property. It does not model Rust atomics, weak memory, real stack switching, general cancellation/wake arbitration, or every callback. No invalid pointer is dereferenced. The token control proves only that the consumed wrapper cannot be reused; obtaining a token from unrelated state remains possible. It does not enforce a real scheduler publication boundary. Rust ownership helps enforce the chosen representation; it does not automatically provide the actor contract.

## Reproduction

Restore archived tools and selected pinned source inputs if necessary:

```sh
python3 experiments/rust-green-history/prepare.py
```

Run only correctness probes with a fresh label:

```sh
python3 experiments/rust-green-history/run.py --label my-correctness-run
```

The runner retains source inputs, source manifests, compiler/runtime artifact hashes, command output and exact expected JSON. It builds historical code with default optimization and `-O`; modern code uses optimization levels 0 and 3. These profiles check semantics under optimization and are **not compared for performance**. Both reject controls run under both profiles. There are 26 expected command outcomes and 20 observations:8 historical runtime,8 modern runtime,4 compiler rejections.

The modern compiler path is explicit in `run.py`; update it deliberately on another host. Watchdogs only detect hangs; the runner records no elapsed duration. No combined evidence runner or benchmark is invoked.

Read-only evidence audit:

```sh
python3 evidence/rust-green-history/validate.py
```

Early manual development caught old syntax/type differences before the recorded cohort. Recorded attempt `20261009-green-01` failed before any experiment because Rust 0.12 does not accept the modern `-C opt-level` option. `20261009-green-02` uses historical `-O` correctly and is the passing cohort. No failed attempt is counted as passed evidence.
