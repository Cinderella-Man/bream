# Local links and exit trapping

Passed for the restricted local single-worker profile, 2026-10-08. [Implementation and limits](README.md), [final cohort](../../evidence/link-process/runs/20261008-link-02/results.json), [audit](../../evidence/link-process/validation.json).

All 263 expected command outcomes passed: 68 unit tests per debug/release profile, six link controls plus six each of the inherited mailbox/timeout/monitor controls, 210 expected compilation rejections and twelve positive shadowing executions, plus builds. Link controls include 96 link/kill and 96 unlink/kill races. Logical worker checks establish fatal retirement without another application poll and peer execution between cleanup batches. The second cohort strengthens the test of the shared eight-item allowance when cleanup moves from links to monitors; the first passing cohort remains retained.

One weak shared edge represents each exact process pair. Cancelled absent-target work is removed and pending-work wakes are coalesced. These are structural ownership/bookkeeping choices, without measured speed claims. No speed tests or performance comparisons ran. Earlier implementation and post-removal correctness snapshots remain unchanged.

G1–G4 remain partial. Atomic monitored/linked spawn is next. Direct exit APIs, arbitrary reasons, real-time timer driving and the room/session fixture remain open.
