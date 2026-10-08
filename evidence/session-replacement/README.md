# Session replacement correctness evidence

The [twelfth milestone](../../experiments/session-replacement/README.md) exercises a bounded room-member replacement using existing checked primitives. The [runner](run.py) executes correctness controls only. No speed test, throughput/latency measurement or performance comparison runs; watchdogs only detect hangs and incidental Cargo duration text is not analyzed. [Machine constraint](../../AGENTS.md).

The [passing cohort](runs/20261008-replacement-01/results.json) retains 387 expected command outcomes, 352 structured observations and 187 source/runner inputs copied before execution. It preserves tool/binary identities, exact commands and raw output. Debug and release each use two build jobs. Existing cohort names are refused.

Each profile includes 95 inherited runtime tests, three configurations of nine public control families (mailbox, timeout, monitor, link, spawn, room, sharing, busy member and replacement), 148 expected compiler rejections and eight positive shadowing executions. Runtime/compiler semantics are inherited; the new coverage is composition of those contracts in a replacement protocol.

The six replacement configurations cover twenty-four scenarios: continuing after old-member normal exit; continuing after old-member contained failure; explicit room kill after installing the replacement; and active runtime shutdown at the same boundary. A registry of capacity three is full with room and two members. Only old-member termination frees a slot, so successful new-member publication necessarily reuses that slot. This is inferred from enforced capacity and live counts, rather than a recorded numeric slot address. The checked program separately requires distinct full PIDs and monitor references.

A genuine old-member report and exact Down are queued before the host supplies replacement input. The host also queues one old-epoch request. Selective receive leaves those three events pending while the replacement reports readiness. Plain demonitor of the retired reference must preserve old queued Down and leave the new monitor intact. A Word buffer and shared handle sent to the old PID must be discarded, with their original ownership receipts released. Subsequent successful operation detects any mistaken delivery to the replacement.

`transcripts` contains actual room checkpoints for the normal and failure cases, each `[stage, progress, result, queued_events]`:

| Stage | Observed meaning |
|---|---|
| 10 | Survivor replied before replacement: progress1, result626271, queue0. |
| 11 | Old report and Down preserved: progress1, prior result, queue2. |
| 20 | Replacement ready with three stale events still queued: progress1, prior result, queue3. |
| 30 | Replacement reply consumed: progress111 encodes one old report, one exact old Down and one rejected old request; result626272; queue0. |
| 40 | Survivor replied after replacement: progress112, result626273, queue0. |

Members compute base31 checksum62627 from bytes `[2,3,5,7]`; a room result is the actual received checksum multiplied by10 plus its sequence. The old report uses sequence99. Normal/failure Down reasons are checked inside the admitted application; the two transcript arrays are ordered by those modes. Checksums are bounded content oracles. Final close receives a child acknowledgement and its exact normal Down, but does not separately validate acknowledgement contents.

`ownership` retains `[live_capacity, freed_capacity, storage_drops, live_refs, created_refs, dropped_refs]` before and after final host release. Continuing scenarios create six references: host, room, two initial members, replacement and a deliberately discarded stale-PID share. Five drop before the host; its final drop frees the four-byte backing once. Teardown scenarios add nineteen queued shares, giving twenty-five created/dropped references. The host directly reads all four bytes before final release.

At termination, three old events plus nineteen shared envelopes total twenty-two queued events. `terminal_rows` is `[explicit_room_kill, terminal, cleanup_messages, cleanup_batches]`, requiring `[0,2,22,3]` for active shutdown and `[1,2,22,3]` for explicit kill. The latter requires both remaining members to leave live lookup through their room monitors before final runtime shutdown. All original Words allocations and shared references must settle once.

Scalar epochs are trusted application protocol labels, not authenticated sender identities. Old Down is already published before replacement; application consumption is deferred. No global order among independent senders is inferred, and priority reception is outside this profile. Independent atomic receipt fields are compared at established ownership boundaries.

```sh
python3 evidence/session-replacement/run.py UNIQUE_NEW_CORRECTNESS_RUN_NAME
python3 evidence/session-replacement/validate.py
```

The [auditor](validate.py) checks retained hashes, current inputs, expected command outcomes, compiler diagnostics, positive outputs, native checkpoints, cleanup and ownership rows. It verifies prior busy-session inputs/evidence without rewriting them and checks eighteen inherited runtime/compiler files after normalizing crate names. It also verifies the pinned OTP checkout and invokes the [performance-reset audit](../performance-reset/README.md). The audit runs no build, runtime fixture or benchmark. [Audit result](validation.json).

Remaining scope includes general membership collections, repeated replacement policies, authenticated application senders, direct checked exits, richer reasons, real-time timers, multiple workers and complete G1–G5 qualification. Future speed qualification requires a suitable separately authorized environment.
