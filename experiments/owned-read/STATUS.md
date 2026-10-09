# Runtime-owned local read

Passed as milestone fourteen for the declared local single-worker profile, 2026-10-09. [Contract and limits](README.md), [cohort](../../evidence/owned-read/runs/20261009-read-01/results.json), [audit](../../evidence/owned-read/validation.json).

The full run passes 454 expected command outcomes, 408 aggregate observations and 219 frozen inputs. Each compiler profile passes 119 runtime tests, 169 rejected consumers and ten positive shadowing examples. Six public read configurations include 96 read/kill races. The previous milestone's 205 inputs remain unchanged. Review found and fixed an early cancellation sample and an unbounded interrupted-wake retry; regression controls pass.

One host-installed nonblocking UnixStream read per actor, with a runtime-owned endpoint and buffer, typed checked result, bounded pending/ready admission, exact-incarnation routing and separately observable resource retirement. Actor stop does not join the driver or free claimed storage. Completing the first read closes the endpoint.

No speed tests or performance comparisons. The backend remains a restricted single-application-worker experiment with one optional driver thread, trusted host endpoint exclusivity and no general native cancellation guarantee.
