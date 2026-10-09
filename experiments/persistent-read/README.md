# Persistent local reads with separate request identities

**Milestone 15 passed for the declared profile on 2026-10-09.** This extends the [runtime-owned read milestone](../owned-read/README.md) with an endpoint that survives positive reads. One actor owns the endpoint; each read has a separate, nonwrapping request identity. [Pinned source contract](../../docs/research/68-persistent-read-source-contract.md), [implementation findings](../../docs/research/69-persistent-read-milestone.md). Broader G1–G5 qualification remains open.

**Correctness only:** no speed tests, throughput/latency measurements or performance comparisons. Watchdogs only detect hangs. The user runs other work on this machine. [Standing instructions](../../AGENTS.md).

## Declared public profile

The host creates `Runtime::with_stream_capacity(allowance, process_capacity, endpoint_capacity)` and consumes a local UnixStream through `client.install_stream(pid, endpoint, chunk_size)`. Admission requires a live full namespace/slot/generation, no accepted fatal exit, one installation per actor, an endpoint slot and a chunk size from 1 to 4096 bytes. The host must preserve the admitted side's nonblocking mode and avoid competing reads through previously created aliases. Installation does not itself read or allocate a payload buffer.

Checked application `read_next()` begins one request and waits for `StreamResult`:

| Result | Meaning |
|---|---|
| `Data(StreamBytes)` | A positive read completed. The existing allocation moves into the actor; the endpoint remains open, and any remaining stream bytes are available to a later request. |
| `Eof` | A nonempty read returned zero. This profile closes the endpoint and caches EOF; subsequent calls return EOF without another request or buffer allocation. |
| `Failed` | A read error other than WouldBlock/Interrupted occurred. The endpoint closes and failure is cached; errno detail is omitted. |
| `Unavailable` | No stream is installed, closure/cancellation made this request unavailable, or request identity space was exhausted. |
| `Busy` | The endpoint already has a pending, running or ready-but-unconsumed request. Checked code awaits each operation and does not expose arbitrary futures; internal controls exercise this guard directly. |

`stream_len(bytes)` and `stream_byte(bytes, index)` provide immutable access. StreamBytes move into checked records/helpers but cannot be cloned, fabricated or passed as the existing shared-byte envelope. The previous `read_input()` one-shot path remains available with its previous semantics in a separate runtime configuration.

The dedicated driver owns at most sixteen endpoint records, including idle endpoints and completed but unconsumed requests. Each endpoint admits one request at a time. Consuming a result releases that request's admission, not the endpoint slot. Delivered buffers can coexist with later request buffers and remain counted until dropped; endpoint capacity is not a whole-runtime memory cap.

This is read-some, not read-exactly or application message framing. Writes, endpoint transfer, half-close, socket options and public read cancellation/timeouts are outside this milestone. Closing the entire endpoint on EOF/error is Bream's narrower policy; it is not claimed to reproduce every BEAM socket branch.

## Ownership and request arbitration

The [persistent adapter](runtime/src/stream_read.rs) separates an endpoint record from the active request. Starting a request advances a checked u64 identity and allocates its bounded buffer. Exhaustion closes the endpoint rather than wrapping to an old identity. A completed future remembers completion, so its destructor cannot cancel a later request.

Driver claim moves the endpoint and pending buffer into a private request and releases shared-state locks before the synchronous nonblocking read. Until that request returns, the endpoint stays busy even if cancellation was requested. Positive completion returns the endpoint to its record and places the owned buffer in the matching result. EOF/error disposes endpoint and request buffer, then publishes the corresponding terminal result. WouldBlock/Interrupted retains the same identity and buffer for a later readiness dispatch.

Readiness tickets carry both the endpoint record and request ID. Dispatch, cancellation and completion check that ID; completion additionally requires the matching Running phase. An old event cannot satisfy or cancel a newer request on the same live actor. A synthetic stale completion also disposes its own supplied resources without replacing the current result. These checks are needed in addition to the process generation checks inherited from earlier milestones.

Actor stop only marks closure and wakes the driver. It never closes a descriptor in another thread's poll snapshot or frees a claimed buffer. Only the sole driver changes descriptor ownership. Snapshot references retain the record, while this sole-driver rule supplies the actual descriptor lifetime guarantee. A claimed read that passed its cancellation check may consume bytes after actor death, then discard them; cancellation is not rollback.

Internal request cancellation is intent, not a synchronous revocation barrier. A ready result can win consumption before the driver retires it. No separate checked cancellation API is exposed. A future timeout/cancellation API must define its winner and acknowledgment rules explicitly.

## Driver, stop and shutdown

The driver attempts at most one bounded read per readiness dispatch and retries Interrupted through driver control processing. A private nonblocking wake socketpair reports admission, request and closure changes. A full wake socket already contains a wake; any other write failure closes the sole writer, causing persistent EOF/HUP and fail-closed driver settlement. There is no unbounded wake-write retry under actor lifecycle locks.

Wait enrollment and publication use the mailbox/state lock protocol; the driver releases request state before notification. Host shutdown closes actor admission, joins scheduler cleanup, closes the stream hub and joins the driver from the host. Open remote peers do not need to close to release idle or waiting resources. Worker/driver panic recovery, multiple application workers and a wall-clock latency bound remain unqualified.

A test-only gate captures the driver's descriptor snapshot immediately before a real poll call. Actor cleanup completes while that snapshot remains held and a peer write still succeeds. The driver then executes poll, observes no invalid descriptor, and retires the endpoint afterward. This qualifies a captured pre-poll snapshot boundary; it does not assert that the kernel had already blocked inside poll when stop occurred.

## Executed controls

The [passing cohort](../../evidence/persistent-read/runs/20261009-stream-01/results.json) records 484 expected command outcomes and 434 aggregate observations. Each debug/release profile passes 136 runtime tests, 179 rejected consumer programs and eleven positive shadowing examples. Six public stream configurations (allowances 1, 4 and 32 in both profiles) include 96 second-read/kill races. All inherited controls pass. The [read-only audit](../../evidence/persistent-read/validation.json) verifies 1,233 hash records, 233 current inputs and preservation of all 219 preceding-milestone inputs.

The [public application](consumer/src/streams.rs) holds the first returned buffer in a helper while issuing a second read, then validates both buffers after EOF. One case drains a four-byte chunk followed by three already buffered bytes; another gates separate writes and completes peers while reads wait. Request IDs are 1 then 2; the EOF request is 3, and a repeated EOF allocates nothing.

Six stop cases cover: idle before the first request; waiting for the first read; holding its result between reads; waiting for the second read; holding both results; and waiting for the EOF/third request. Each settles nineteen unrelated queued Words messages in three cleanup batches, tracks endpoint/buffer release separately and requires frozen application observations after stop.

Public controls also retain endpoint capacity after a consumed read, release it after owner death, force same-slot process replacement, shut down idle/pending/second-read endpoints with peers open, and race a second write with actor kill. The internal tests cover old cancellation/completion/readiness against a newer Running request, cancellation while claimed, cached failure, identity exhaustion, abandoned futures, capacity, controlled poll ownership and failure of the wake channel. The buffered-data unit closes its peer before draining 4+3+EOF.

Synthetic failure and duplicate-completion controls are labeled as such; they do not induce arbitrary native read errors or duplicate kernel callbacks. The [evidence guide](../../evidence/persistent-read/README.md) records retained results. No direct equivalence to an Erlang socket API or general native I/O cancellation is claimed.

Next: a bounded read timeout using the existing injected clock, with a defined data/timeout winner and cancellation settlement before endpoint reuse. Preserve old timer/request identity separation and exact cleanup without interpreting watchdog durations as latency evidence.
