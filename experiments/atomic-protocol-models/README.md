# Atomic protocol projections

A standalone research executable using pinned `loom = 0.7.2`, not a Bream library implementation. The three projections examine payload publication, visibility at final-owner release and a parking/wakeup handshake. They deliberately avoid raw pointers and use atomic payload fields so failures are invariant violations, not intentional undefined behavior.

Run and retain evidence with `python3 evidence/concurrency-clock-review/run_models.py` from the Bream project root. [Report49](../../docs/research/49-rust-atomic-protocol-observations.md) explains the results and bounds; [evidence](../../evidence/concurrency-clock-review/README.md) records reproducibility details. `Cargo.lock` pins dependencies; the runner supplies the local Rust toolchain and keeps build artifacts in ignored `.tools/atomic-protocol-models/`.

All protocol-visible synchronization uses Loom types. The sole ordinary atomic is an observation counter incremented once per model closure invocation; its value never controls the algorithm. No explicit yields or async polls are inserted in subject code. Loom's own schedule exploration is test machinery, not an application scheduling API.

These projections do not represent process kill, raw allocation retirement, a full mailbox, epoch algorithms, weak-memory completeness or fair scheduler execution. Passing models do not qualify Bream; four negative controls intentionally terminate with their named invariant failures.
