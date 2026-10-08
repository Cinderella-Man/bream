# Worker notification experiment

**Performance evidence removed, 2026-10-08, at the user’s request. All speed tests must be redone in an authorized suitable environment. No speed tests may run on this machine.**

Research-only queue notification comparison and four tiny Loom controls. This is not a Bream scheduler or an implementation of process preemption, forced termination or runtime-aware I/O.

[Report57](../../docs/research/57-worker-notification-models-and-load-observations.md) interprets the results; [report54](../../docs/research/54-worker-sleep-wake-and-controller-boundaries.md) traces the BEAM source obligations.

- `src/model.rs`: one-item publication/closure projections, two positive variants and two intentional deadlock controls.
- `src/probe.rs`: first saturated/sparse/burst queue comparison.
- `src/controlled.rs`: preserved first equal-arrival follow-up.
- `src/controlled_ready.rs`: final follow-up with allocation-before-start barrier, balanced order in its runner, intended-arrival delay and queue high-water.

The repeated source files preserve exactly the compiled inputs for separate evidence stages; they are throwaway mechanism probes, not a proposed production module structure. All native variants keep the same mutex-owned queue predicate and drain admitted work on close. The park enrollment-to-wait interval contains no user callback or logging.

[Replay commands and evidence](../../evidence/worker-notification-review/README.md). No benchmark result selects a backend or completes a Bream performance task.
