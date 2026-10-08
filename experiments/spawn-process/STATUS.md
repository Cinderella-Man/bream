# Atomic monitored/linked spawn

Passed for the restricted local single-worker profile, 2026-10-08. [Implementation and limits](README.md), [final cohort](../../evidence/spawn-process/runs/20261008-spawn-02/results.json), [audit](../../evidence/spawn-process/validation.json).

All 311 expected command outcomes passed: 83 unit tests per debug/release profile, six spawn controls plus six each of the inherited mailbox/timeout/monitor/link controls, 248 expected compilation rejections and fourteen positive shadowing executions, plus builds. Spawn controls include 96 spawn/parent-kill and 96 spawn/shutdown races. A real worker checks initial relationships on its first child poll. Deterministic controls cover both sides of publication/stop/shutdown, monitor identity exhaustion, dispatch disconnection and rollback without spurious notifications.

The second cohort strengthens the operand evaluation oracle with ordered messages; the first passing cohort remains retained. Child input moves without payload copying and keeps its original allocation receipt. No speed tests or performance comparisons ran. Earlier implementation and post-removal correctness snapshots remain unchanged.

G1–G4 remain partial. A bounded room/session fixture is next. Direct exit APIs, arbitrary reasons and real-time timer driving remain open.
