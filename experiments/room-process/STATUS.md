# Bounded room/session integration

Passed for the fixed two-session single-worker profile, 2026-10-08. [Application and limits](README.md), [cohort](../../evidence/room-process/runs/20261008-room-01/results.json), [audit](../../evidence/room-process/validation.json).

All 317 expected command outcomes passed. Six room configurations cover eighteen conversation/room-death/shutdown scenarios, retaining the expected fifteen-record transcript and actual collector digest/count/sequence checkpoints. Inherited checks pass: 83 unit tests per compiler profile, mailbox/timeout/monitor/link/spawn controls, 248 expected compiler rejections and fourteen positive shadowing executions.

The room uses exact monitor identities, survives one failed member, continues routing to the other member beside an independent CPU loop, preserves a queued publish across logical timeout, and settles held/queued allocations during death and host shutdown. Review strengthened session room-death and input-accounting oracles before the cohort. Runtime/compiler semantics are unchanged apart from crate binding names. No speed test or comparison ran; prior implementation and correctness evidence remain unchanged.

G1–G5 remain partial. Runtime-managed immutable payload sharing and bounded zero/one/two-recipient broadcast are next. General membership, a hogging session, direct exit APIs, arbitrary reasons and real-time timers remain open.
