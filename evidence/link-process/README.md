# Link and exit-trapping correctness evidence

This [seventh milestone](../../experiments/link-process/README.md) adds local bidirectional links and genuine Exit notifications to the checked single-worker profile. The [runner](run.py) executes correctness controls only. No benchmark or throughput/latency measurement runs; watchdogs detect hangs and incidental Cargo duration text is not analyzed. [Machine constraint](../../AGENTS.md).

Each new cohort copies source and writes the protocol before execution, preserving tool/binary identities, exact commands, raw output and expected results. Both compiler profiles use two build jobs. Existing cohort names are refused.

The protocol requires 68 unit tests per profile, three mailbox/timeout/monitor/link configurations per profile, 105 expected rejected consumers per profile and six positive shadowing examples per profile. Link configurations include 16 link/kill and 16 unlink/kill races each; they permit all declared outcomes and do not require a particular race winner. Deterministic tests separately force conversion, cancellation, trap-change, relink and incarnation-reuse orders. Actual-worker tests establish peer execution between link-cleanup batches and retirement of a pending fatal process without another application poll.

The failure controls intentionally trigger an out-of-bounds admitted read, invalid receive duration, deadline overflow and uncaught `link` to an absent target. These panics must be contained by the process worker and followed by the expected reason/ownership checks; their stderr is not an unexpected test failure.

```sh
python3 evidence/link-process/run.py UNIQUE_NEW_CORRECTNESS_RUN_NAME
python3 evidence/link-process/validate.py
```

The [read-only validator](validate.py) checks retained file hashes, current input identity, raw observations, command exits and compiler diagnostics. It also checks both earlier monitor cohorts against their saved hashes without rewriting them, then runs the [performance-reset audit](../performance-reset/README.md). The audit does not execute a compiler, runtime, model or benchmark.

Final cohort: [20261008-link-02](runs/20261008-link-02/results.json), with all 263 expected command outcomes passing. The [first passing cohort](runs/20261008-link-01/results.json) remains retained. Final review strengthened the combined cleanup-budget test to cross phases with ten links and ten monitors: batch two processes two links and six monitors, leaving four monitors. [Audit status](validation.json) verifies both retained cohorts and current inputs.

Limits: one application worker, logical-clock receive timeouts, concrete payloads and genuine lifecycle events, trusted allocator/panic handling, restricted reason codes. Direct exit APIs, arbitrary exit terms, atomic relationship spawn, distribution, general exception catching and full G4 qualification remain open. No speed result or production latency guarantee is inferred.
