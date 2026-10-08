# Receive timeouts with injected time

Passed for the restricted single-worker logical-clock profile, 2026-10-08. [Implementation and limits](README.md), [final correctness cohort](../../evidence/receive-timeout/runs/20261008-timeout-04/results.json).

All 183 expected command outcomes passed: 35 unit tests per compiler profile, six inherited mailbox controls, six timeout controls, 150 expected compilation rejections and eight positive shadowing executions, plus builds. Timeout controls include 96 send/timeout races and 96 kill/timeout races. The format-equivalence regression first failed, then passed after restoring the nested match's generated checkpoint.

No speed tests or performance comparisons ran. Earlier experiments/evidence and the failed/superseded new cohorts remain unchanged. G1/G2/G3 remain partial; the runtime has no real-time timer driver. The next bounded milestone is monitors/Down with creation/death ordering, cancellation, owner cleanup and separate final reclamation.
