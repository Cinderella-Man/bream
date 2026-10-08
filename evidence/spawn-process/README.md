# Atomic-spawn correctness evidence

This [eighth milestone](../../experiments/spawn-process/README.md) installs a child's initial link, monitor, or both before dispatch through the checked single-worker profile. The [runner](run.py) executes correctness controls only. No benchmark or throughput/latency measurement runs; watchdogs detect hangs and incidental Cargo duration text is not analyzed. [Machine constraint](../../AGENTS.md).

Each new cohort copies source and writes the protocol before execution, preserving tool/binary identities, exact commands, raw output and expected results. Both compiler profiles use two build jobs. Existing cohort names are refused.

The protocol requires 83 unit tests per profile, three mailbox/timeout/monitor/link/spawn configurations per profile, 124 expected rejected consumers per profile and seven positive shadowing examples per profile. The full cohort has 311 expected command outcomes and 280 structured observations. Spawn configurations include 16 spawn/parent-kill and 16 spawn/shutdown races each; they permit all declared outcomes without requiring a particular race winner.

Deterministic tests separately force initial topology before dispatch, parent stop before/after publication, pending kill before publication, normal versus abnormal parent exit, child trap defaults, immediate committed child reasons, exact identity/reference return, capacity/closed/stale/foreign rejection, monitor identity exhaustion and disconnected rollback. An actual-worker test checks the initial topology on its first child poll. Moved input retains its original allocation receipt and must drop once on rejected publication as well as success.

The failure controls intentionally trigger admitted out-of-bounds reads, a second input take, uncaught spawn-capacity failure and inherited link/timeout errors. The worker must contain those panics and satisfy subsequent reason/ownership assertions; expected stderr is not a cohort failure.

```sh
python3 evidence/spawn-process/run.py UNIQUE_NEW_CORRECTNESS_RUN_NAME
python3 evidence/spawn-process/validate.py
```

The [read-only validator](validate.py) checks retained file hashes, current input identity, raw observations, command exits and compiler diagnostics. It checks both earlier link cohorts against their saved hashes without rewriting them, then runs the [performance-reset audit](../performance-reset/README.md). The audit executes no compiler, runtime, model or benchmark.

Final cohort: [20261008-spawn-02](runs/20261008-spawn-02/results.json). The [first passing cohort](runs/20261008-spawn-01/results.json) remains retained. Final review strengthened the positive shadowing example: three ordered input messages make repeated/reordered operand evaluation affect the result, replacing constants and stage markers that could hide such a mistake. [Audit status](validation.json) verifies retained cohorts and current inputs.

Limits: one application worker, injected logical time, concrete payloads, statically selected local checked children, acyclic helper/spawn graph, trusted allocator and panic behavior, restricted reasons. No arbitrary closures, asynchronous/distributed spawn, general exception recovery or full G4 qualification. No speed result or production latency guarantee is inferred.
