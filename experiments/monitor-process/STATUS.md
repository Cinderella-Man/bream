# Local monitors and selectable Down

Passed for the restricted local single-worker profile, 2026-10-08. [Implementation and limits](README.md), [final cohort](../../evidence/monitor-process/runs/20261008-monitor-02/results.json), [audit](../../evidence/monitor-process/validation.json).

All 221 expected command outcomes passed: 51 unit tests per debug/release profile, six monitor controls plus six mailbox and six timeout controls, 178 expected compilation rejections and ten positive shadowing executions, plus builds. Monitor controls include 96 creation/kill and 96 cancellation/kill races. The second cohort adds an actual-worker logical check proving peer execution between monitor cleanup batches; the first passing 50-test cohort remains retained.

Review eliminated redundant monitor service commands and cancelled absent-target tombstones before qualification. No speed tests or performance comparisons ran. Earlier implementation and post-removal correctness snapshots remain unchanged. G1–G4 remain partial. Local links/exit trapping are next; real-time timer driving, atomic relationship spawn and the room/session fixture remain open.
