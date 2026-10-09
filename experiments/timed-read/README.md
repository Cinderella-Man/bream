# Persistent local reads with injected-clock timeouts

**Milestone 16 passed for the declared profile on 2026-10-09.** This extends the [persistent-read milestone](../persistent-read/README.md) without changing that earlier workspace or evidence. [Pinned BEAM source contract](../../docs/research/70-timed-read-source-contract.md). The scope remains one process owner per endpoint, one application worker and one dedicated driver; broader G1–G5 qualification is partial.

**Correctness only:** no speed tests, throughput/latency measurements or performance comparisons. Watchdogs detect hangs. [Machine constraint](../../AGENTS.md).

## Checked operations

The host uses the existing `Runtime::with_stream_capacity` and consumes a local UnixStream through `install_stream`. The endpoint retains the existing fixed chunk size of 1–4096 bytes, sixteen-endpoint maximum, one active request per endpoint and trusted-host restriction against competing reads or changes to nonblocking mode.

Checked `read_next_for(ticks)` returns `TimedStreamResult` with six alternatives: `Data(StreamBytes)`, `Eof`, `Failed`, `Unavailable`, `Busy` and `Timeout`. StreamBytes retain the existing move-only immutable access and checked-record/helper support. Arbitrary futures, user callbacks and fabricated result values remain outside admission. Existing `read_next()` remains indefinite and returns the unchanged five-variant StreamResult.

| Duration | Behavior |
|---|---|
| Zero | One nonblocking attempt when the driver dispatches it. No timer or readiness wait; WouldBlock/Interrupted becomes Timeout. The process can suspend while waiting for that driver attempt. |
| 1 through u32::MAX | Capture one deadline from the injected clock when the request starts. Retries and unrelated wakes preserve it. |
| Indefinite | Use `read_next()`; no timer is registered. |

A larger duration is rejected by a panic before request admission. Deadline addition overflow also rejects before publishing a request or consuming its identity, without poisoning state locks. These follow the experiment's existing failure boundary; no general exception-catching API is added. Unlike mailbox receive's initial scan, finite stream reads register before their first attempt, even if data is already buffered. Terminal cached EOF/failure and unavailable/busy results register no timer.

The host advances logical time with `RuntimeClient::advance_to`. Advancing time makes callbacks eligible; the existing scheduler services at most eight per turn. There is no real-time clock driver or elapsed-time guarantee.

## Arbitration and settlement

The [adapter](runtime/src/stream_read.rs) makes timeout decisions under the endpoint-state lock, against the exact operation identity:

- A pending request expires to a committed Timeout; its buffer is released and the endpoint remains open. An old readiness ticket cannot claim that request afterward.
- Expiry during a claimed request records intent. Its future remains pending, its storage stays owned by the driver, and a second request remains Busy until settlement.
- The driver samples stop/cancel/expiry after claim. Expiry observed there can suppress the syscall. If the check already passed, positive data, EOF or failure wins; WouldBlock/Interrupted becomes Timeout.
- A ready result already committed cannot be replaced by expiry. Completion removes its timer even if the process has not consumed the result.
- Process stop or future destruction still suppresses delivery, including data consumed by a claimed read. Timeout preference never overrides process death.

Receiving Timeout therefore establishes that the old read has settled and the next read can obtain a fresh request identity. It does not restore bytes consumed by cancelled work or promise a strict completion cutoff. The source rationale is OTP's shared receive/cancel read lock; Bream implements explicit Running-state ownership instead. This is a narrower policy, not exact socket:recv equivalence.

Only the driver closes or moves descriptors, preserving the preceding poll-snapshot lifetime rule. Pending expiry can dispose its record-owned buffer but does not touch the endpoint. Close on EOF/error remains the earlier read-only policy. Writes, half-close and transfer are outside this milestone.

## Timer ownership and allocation

The [timer service](runtime/src/timer.rs) now distinguishes mailbox and stream destinations in both its ordered key and typed weak target. Equal process, deadline and generation values cannot alias the two domains. A stream callback checks the full key and current request identity. Detached old callbacks cannot expire a later request or a replacement process.

The request owns its timer registration; completion, abandoned futures and driver retirement remove it. Timer service releases the clock lock before calling an endpoint or mailbox. Endpoint completion releases its state lock before notifying the mailbox, preserving the established lock order. Stop can precede final driver retirement; host shutdown joins both scheduler cleanup and resource settlement.

Stop and expiry flags share one Arc allocation per request, avoiding a second separately allocated flag. Completed buffers still move without another payload copy. Each request nevertheless allocates a buffer and signal record; finite timers also occupy the ordered timer index. These are structural observations, not measured speed results. Delivered buffers may outlive requests, so endpoint capacity is not a whole-runtime memory bound.

`ClockSnapshot.expired` counts accepted expiry callbacks, including intent while a claimed read later returns data. `StreamSnapshot.timeouts` counts committed Timeout results, including results later discarded on stop. Neither means the application necessarily observed Timeout. Ownership counters are separate atomics; compare balances at controlled settlement/retirement boundaries.

## Validation and limits

The [passing cohort](../../evidence/timed-read/runs/20261009-timed-02/results.json) records 517 expected command outcomes, 462 aggregate observations and 247 retained inputs. Each debug/release profile passes 153 runtime tests, 189 expected compiler rejections and twelve positive shadowing examples. Six public configurations cover 96 data/timeout and 96 kill/timeout races. Both BEAM emulator profiles pass six new socket cases and the inherited thirty-nine exit cases. The [audit](../../evidence/timed-read/validation.json) verifies 1,651 hash records across two retained cohorts and preservation of all 233 preceding-milestone inputs.

The [first cohort](../../evidence/timed-read/runs/20261009-timed-01/results.json) exposed an inherited exit-fixture synchronization error: its baseline snapshot preceded the worker's park counter update. A completed peer now establishes the prior turn's completion before the comparison. The failure remains retained; the original earlier workspace is unchanged. [Implementation findings](../../docs/research/71-timed-read-milestone.md).

[Public controls](consumer/src/timed_reads.rs) cover zero empty/data/short/EOF, finite empty/data, indefinite data, and a data → timeout → data → EOF sequence retaining earlier bytes. Seven stop boundaries dispose nineteen queued messages in three cleanup batches. Shutdown closes idle and waiting endpoints with peers open. Data/timeout and kill/timeout races accept the declared alternatives and require exact ownership settlement.

Seventeen [new internal tests](runtime/src/stream_read/timed_tests.rs) force detached callbacks, old readiness, claimed outcomes, same-slot replacement, drop, overflow, timer-domain collisions and bounded callback service. The failure and Interrupted branches use injected outcomes; they do not induce arbitrary native errors. Ten new rejected consumers and a positive shadowing example qualify the new checked vocabulary.

The [six-case OTP fixture](timed_read_reference.erl) tests zero, finite timeout with endpoint reuse, infinity, stale cancellation and an intentional difference: OTP exact-length receive can report partial bytes with timeout, whereas Bream reports positive short Data. It reads no clock or duration. It does not reproduce the injected-clock race matrix. [Evidence guide](../../evidence/timed-read/README.md).

General native cancellation, hard deadlines, driver-panic recovery, multiple workers and production performance remain unqualified. Next: bounded runtime-owned writes on the same endpoint, starting with consuming a returned StreamBytes buffer to support a small echo exchange. Partial writes and stop must retain storage until settlement and expose irreversible external effects explicitly.
