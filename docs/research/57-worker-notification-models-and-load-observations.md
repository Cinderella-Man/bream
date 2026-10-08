# Worker notification models — timing results removed

**Performance evidence removed, 2026-10-08, at the user’s request. All speed tests must be redone in an authorized suitable environment. No speed tests may run on this machine.**

All queue timing stages, raw samples, warmups, throughput/latency summaries and comparisons have been deleted. PERF-100 and PERF-101 need fresh performance qualification. No notification backend is selected.

Four finite Loom model observations remain in the [filtered results](../../evidence/worker-notification-review/results.json): `condvar_locked` and `park_locked` complete; `park_split_enrollment` and `park_token_consumed` expose the intended deadlocks. The [model source](../../experiments/worker-notification/src/model.rs) checks a small publication/closure projection. It is not a complete Bream runtime or a proof covering all spurious wakes, scheduling, cleanup or host behavior.

The [native experiment sources](../../experiments/worker-notification/README.md) remain as inputs for redesigned experiments. Queue state must stay protected, wake notification must not be confused with permission to execute, and timer/control/cleanup work must retain progress. [Pinned BEAM source analysis](54-worker-sleep-wake-and-controller-boundaries.md).

Future comparison must control offered load, startup and order, and include admitted-process peer and stop behavior. These are requirements for a new experiment, not retained findings about speed. [Removal record and correctness audit](../../evidence/performance-reset/README.md).
