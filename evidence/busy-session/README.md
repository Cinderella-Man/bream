# Busy-session correctness evidence

The [eleventh milestone](../../experiments/busy-session/README.md) puts sustained admitted computation inside a genuine room member. The [runner](run.py) uses bounded correctness tests only. No speed test or performance comparison runs; watchdogs detect hangs and incidental Cargo duration text is not analyzed. Logical progress is never converted to rates or compared across allowances. [Machine constraint](../../AGENTS.md).

The [passing cohort](runs/20261008-busy-01/results.json) records 381 expected command outcomes, 346 structured observations and 186 source/runner inputs copied before execution. It preserves tool/binary identities, exact commands and raw output. Debug and release each use two build jobs. Existing cohort names are refused.

Each profile includes 95 inherited runtime tests, three configurations of eight public control families (mailbox, timeout, monitor, link, spawn, room, sharing and busy member), 148 expected compiler rejections and eight positive shadowing executions. This milestone adds application integration coverage; it does not add runtime/compiler semantics or claim those inherited tests as new primitive coverage.

The six busy-member configurations cover eighteen scenarios: direct busy-member kill, fatal linked exit and active runtime shutdown. The room monitors a host-created busy member and broadcasts a shared payload to it and a checked child. Each scenario completes three room/healthy-child requests while the busy member stays runnable and advances its logical round count. The busy loop does not receive or test for stop. Both explicit-stop scenarios then complete two further healthy-child replies, with exact Down already consumed and the busy application's state frozen.

Observed fields are retained separately from assertions:

| Field | Meaning and independently checked expectation |
|---|---|
| `progress_observations` | Three scenarios, each with three rows `[room_sequence, room_result, busy_round_before, busy_round_after]`. The room result is `626270 + sequence`; the busy round strictly increases from at least 2. No elapsed time or work-per-time value exists. |
| `post_stop_observations` | `[link_mode, sequence, actual_room_result]` for sequences 4 and 5 in direct and linked stop. The survivor reads the payload and replies after busy cleanup. |
| `stop_observations` | `[link_mode, terminal, reason, receives, cleanup_messages, cleanup_batches, busy_checksum]`. Direct terminal 2 and linked terminal 3 both have reason 2, one receive, 22 queued releases, 3 batches and checksum 62627. |
| `ownership_observations` | For each explicit-stop scenario: after busy cleanup, after all process cleanup, and after the final host drop. Each row is `[live_capacity, freed_capacity, storage_drops, live_refs, created_refs, dropped_refs]`. |
| `shutdown_ownership` | The same last-host and final-free observations under active shutdown. |
| `shutdown_cleanup`, `shutdown_busy_result`, `shutdown_room_result` | Queue cleanup `[22,3]`, busy checksum 62627 and room result 626273 after shutdown. |

The payload is `[2,3,5,7]`, with base31 digest 62627. The busy helper increments each byte in scratch storage and subtracts it back while constructing the digest. The healthy reader returns that digest using the original request's owned Words allocation. Checksums are bounded content oracles, not collision-free proofs; the host also compares all four bytes directly at final ownership.

There are 23 shared references after nineteen extra shared envelopes are queued: host, room, two members and nineteen messages. Three ordinary queued messages add no shared references. Busy cleanup must drop 20 references (its held reference plus 19 queued); root and healthy-child cleanup leave one host owner; host drop then yields 23 created/23 dropped and one storage destruction. The count cannot grow during repeated owning-helper calls. Ordinary inputs and request/message receipts must settle once too.

Quiescent checks distinguish stopped execution, exact Down, tracked process cleanup and final shared-storage release. The active-shutdown scenario does not require a room application to consume Down after shutdown. Bounded samples establish these observed schedules and ownership obligations, not a universal fairness bound or numeric stop latency.

```sh
python3 evidence/busy-session/run.py UNIQUE_NEW_CORRECTNESS_RUN_NAME
python3 evidence/busy-session/validate.py
```

The [auditor](validate.py) verifies all retained hashes, current inputs, command outcomes, compiler diagnostics, shadow outputs and actual native observations. It verifies prior shared-process source/evidence without rewriting them and compares eighteen runtime/compiler files after normalizing crate names. It checks the pinned OTP checkout and invokes the [performance-reset audit](../performance-reset/README.md). The audit executes no build, runtime fixture or benchmark. [Audit result](validation.json).

Remaining scope includes membership replacement and churn, general collections, richer exit reasons, direct checked exit APIs, real timers, multiple workers and full G1–G5 qualification. Any future performance qualification requires a suitable separately authorized environment.
