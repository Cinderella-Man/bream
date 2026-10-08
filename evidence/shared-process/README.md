# Immutable payload sharing correctness evidence

The [tenth milestone](../../experiments/shared-process/README.md) adds sealed immutable bytes, consuming shared messages and bounded broadcast. The [runner](run.py) executes correctness controls only: no speed test, throughput/latency measurement or performance comparison. Watchdogs detect hangs; incidental Cargo duration text is not analyzed. [Machine constraint](../../AGENTS.md).

The [passing cohort](runs/20261008-shared-01/results.json) retains 375 expected command outcomes, 340 structured observations and 185 source/runner inputs copied before execution. It records tool/binary identities, exact commands and raw output. Both compiler profiles use two build jobs; existing cohort names are refused.

Each profile includes 95 runtime tests (twelve new), three configurations each of mailbox, timeout, monitor, link, spawn, room and shared controls, 148 expected compiler rejections and eight positive shadowing examples. The new admission cases reject raw shared types/methods, mutation, forged fields/constructors, ownership reuse and wrong tags/receivers. Missing result/event arms require E0004; duplicate moved owners require E0382. The positive shared example checks explicit clone counts, borrowed access, helper suspension, hygienic paths and argument order.

Six sharing configurations (allowances 1/4/32, debug/release) each cover ten bounded scenarios: zero/one/two-recipient broadcast; one failed recipient; readable survival after publisher kill; readable survival after another reader's kill; cancellation between fanout sends; shared receive timeout preserving a nonmatch; held/queued references on shutdown; and cancellation during a cloned send operand before publication. Each configuration additionally executes sixteen shared-send/kill races, for 96 total. Race results assert allowed ownership outcomes without sampling duration.

`broadcast_observations` contains actual `[recipient_count, injected_failure, result_checksum_sum, acknowledged_count]` values. Each healthy reader computes a base-31 digest of bytes `[2,3,5,7]`, adds its slot, and sends an owning acknowledgement. The room asserts exact monitor/PID identity, normal versus contained-failure reason, and acknowledgement before that child's Down. Cross-child ordering is unrestricted. The independent expected digest is 62627; publisher-death and peer-death observations each retain the surviving known handle's actual digest. Digests are bounded content oracles, not collision-free proofs.

Partial-fanout cancellation qualifies reference accounting: only two references were created, one remains after publisher cleanup, and the second send never occurs. That scenario does not inspect its child checksum; separate known-handle fixtures qualify readable survival. Cancellation inside a suspended send operand requires both references released once and zero target deliveries. Shutdown asserts nineteen queued references released in three cleanup batches while a host owner keeps the backing live. Every final receipt requires balanced created/dropped references and exactly one storage destruction.

Unit controls verify original byte-allocation identity, empty/spare-capacity accounting, the 64KiB capacity limit, receipts that cannot retain payloads, simultaneous final readers, rejected/late delivery, mixed Message/Shared/Exit/Down ordering, typed selection, timeout preservation and eight-event cleanup steps. These are bounded correctness observations, not an exhaustive concurrency proof or elapsed-time bound.

The receiver-failure scenario deliberately reads past the payload length. The worker must contain that panic and the surviving recipient must still acknowledge valid contents. Its expected stderr is retained. Inherited timeout/link/spawn/room controls also contain deliberate failures.

```sh
python3 evidence/shared-process/run.py UNIQUE_NEW_CORRECTNESS_RUN_NAME
python3 evidence/shared-process/validate.py
```

The [read-only auditor](validate.py) verifies retained hashes, current inputs, command outcomes, rejection diagnostics, positive outputs, inherited room digests and new sharing digests. It also checks the prior room cohort and current source without rewriting them, verifies the pinned OTP checkout, and runs the [performance-reset audit](../performance-reset/README.md). The audit runs no build, runtime fixture or benchmark. [Audit result](validation.json).

Limits: one worker, two fixed members, trusted-host byte ingestion, immutable whole buffers, injected logical time and the existing checked grammar. General membership, a hogging session, mutable/sliced/arbitrary shared values, real timers and full G1–G5 qualification remain open. All performance qualification requires an explicitly authorized suitable environment.
