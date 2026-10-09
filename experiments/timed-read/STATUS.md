# Timed persistent local reads

Milestone 16 passed on 2026-10-09. [Contract and limits](README.md), [source audit](../../docs/research/70-timed-read-source-contract.md).

The injected clock can expire a pending read. A claimed read keeps its endpoint and buffer until settlement; a successful claimed result can win serviced expiry. After Timeout is returned, another read can safely begin with a fresh identity. Typed timer destinations isolate stream and mailbox callbacks.

The profile remains one endpoint owner, one application worker and one driver. No speed tests or performance comparisons. No real-time timer, hard deadline, writes, transfer or arbitrary future cancellation is admitted.

The [passing cohort](../../evidence/timed-read/runs/20261009-timed-02/results.json) records 517 expected command outcomes and 462 observations. Each compiler profile passes 153 runtime tests, 189 rejected consumers and twelve positive shadowing examples. Six configurations include 96 data/timeout and 96 kill/timeout races. Two emulator profiles each pass six socket and thirty-nine inherited exit reference cases. The [audit](../../evidence/timed-read/validation.json) passes and preserves all 233 preceding-milestone inputs. The initial failed cohort retains the evidence for the corrected inherited exit-fixture synchronization error.

Next: bounded runtime-owned writes consuming returned StreamBytes on the same endpoint, with partial-write/backpressure, stop and shutdown ownership controls. [Milestone tracker](../../docs/milestones.md).
