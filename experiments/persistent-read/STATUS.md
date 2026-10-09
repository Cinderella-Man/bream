# Persistent local reads

Milestone 15 passed on 2026-10-09. [Contract and limits](README.md), [source audit](../../docs/research/68-persistent-read-source-contract.md), [implementation findings](../../docs/research/69-persistent-read-milestone.md).

A positive read preserves the endpoint; each successive request has a fresh identity. Pending/running/ready requests retain exclusive endpoint use, while consumed buffers follow actor ownership. Old cancellation, completion and readiness are rejected against a newer request. EOF/error closes the endpoint and caches its result.

No speed tests or performance comparisons. The profile remains one actor owner per endpoint, one application worker and one dedicated driver, with trusted host endpoint exclusivity and no writes, transfer or public timed/cancellable reads.

The [passing cohort](../../evidence/persistent-read/runs/20261009-stream-01/results.json) has 484 expected command outcomes, 434 observations and 233 retained inputs. Each compiler profile passes 136 runtime tests, 179 rejected consumers and eleven positive shadowing examples. Six public stream configurations include 96 second-read/kill races. The [audit](../../evidence/persistent-read/validation.json) passes and preserves all 219 preceding-milestone inputs.

Next: bounded read timeouts using the existing injected clock, with explicit data/timeout arbitration and request settlement before endpoint reuse. [Milestone exit conditions](../../docs/milestones.md).
