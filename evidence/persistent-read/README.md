# Persistent-read correctness evidence

The [fifteenth milestone](../../experiments/persistent-read/README.md) keeps a runtime-owned local endpoint across sequential read requests. The [runner](run.py) contains correctness controls only. Watchdogs detect hangs; compiler/test duration text is not analyzed as speed evidence. Two build jobs and two runtime-test threads limit concurrent validation work. [Machine constraint](../../AGENTS.md).

The [retained cohort](runs/20261009-stream-01/results.json) passed on 2026-10-09: 484 expected command outcomes, 434 aggregate observations and 233 copied inputs. Both debug and release pass 136 runtime tests, 179 expected compiler rejections and eleven positive shadowing examples. Twelve public control families run at allowances 1, 4 and 32 in both profiles; the six new stream configurations include 96 second-read/kill races. Inherited BEAM exit controls contribute 78 reference observations across two emulator profiles.

The [read-only audit](validation.json) passes: 1,233 hash records verified, all 233 current inputs match, and all 219 preceding-milestone inputs remain unchanged. The [speed-evidence removal audit](../performance-reset/validation.json) also passes. These counts describe this bounded cohort, not exhaustive concurrency coverage.

## Public result records

Each `stream_controls` output contains two completed-conversation ownership rows:

`[started, settled, delivered, discarded, buffers_allocated, buffers_dropped, live_bytes, endpoint_drops, retired]`

Both must be `[3,3,3,0,3,3,0,1,1]`. The first two requests return four and three bytes; the third reports EOF. Another EOF consumes no request identity or allocation. Actor-held bytes remain readable across the later read and endpoint closure. The application checks checksum 62627 for `[2,3,5,7]`, checksum 10991 for `[11,13,17]`, then combined result 62627010991. The first case records successive request identities `[1,2]`; the host requires final ID 3. A separate unit checks backing-allocation identity across transfer to the actor.

One conversation supplies all seven bytes before the first read, proving the first positive read retains the endpoint and leaves remaining data available. The other supplies the chunks separately and checks independent peer progress during waits. A unit variant closes the remote peer before draining 4+3+EOF, so HUP cannot be mistaken for immediate EOF with buffered bytes remaining.

The six stop rows are:

`[mode, final_stage, requests_started, delivered, discarded, buffers_dropped, endpoint_drops, queued_messages_cleaned, cleanup_batches]`

| Boundary | Expected row |
|---|---|
| Idle before first read | `[0,1,0,0,0,0,1,19,3]` |
| Waiting for first read | `[1,2,1,0,1,1,1,19,3]` |
| First bytes held between reads | `[2,3,1,1,0,1,1,19,3]` |
| Waiting for second read | `[3,4,2,1,1,2,1,19,3]` |
| Both returned buffers held | `[4,5,2,2,0,2,1,19,3]` |
| Waiting for third/EOF read | `[5,6,3,2,1,3,1,19,3]` |

Every stop requires exactly-once buffer/input disposal and nineteen queued Words messages retired in three batches. Independent peers complete afterward; application state remains frozen. The shutdown record uses the completed-row layout for three simultaneous endpoint states: idle, first-read pending, second-read pending with the first buffer owned by the actor. All settle while their remote peers remain open.

Additional controls reject endpoint-capacity admission after a read was consumed, then reuse capacity after owner stop; reject an old PID after forced same-slot replacement; and complete a new endpoint's conversation. Each of six configurations includes sixteen concurrent second-write/kill cases. Either delivery outcome is allowed, while both requests must settle and both buffers must drop once. Winner counts are not interpreted as rates or scheduling quality.

## Controlled internal interleavings

Seventeen new runtime tests complement real socket behavior with explicit phase controls. A held old request survives actor cleanup and slot reuse. Old cancellation, a retained readiness ticket and a stale completion cannot settle a newer Running request. Synthetic duplicate Data owns its own endpoint/buffer; it must dispose them without touching the active request. A cancelled Running request stays Busy until it returns. Cached failure and request-ID exhaustion use injected conditions.

The poll test pauses the real driver after it captured descriptors but before entering poll. Actor stop and frame cleanup complete while the endpoint remains open. After resume, real poll reports no POLLNVAL and the driver then closes the endpoint. This is a captured pre-poll snapshot test, not an assertion that the kernel was already blocked when stop happened. A private wake-writer closure separately injects driver failure and checks settlement of idle/pending records.

Only the driver changes endpoint ownership. Arc record retention by itself is not a general guarantee against another thread closing a descriptor. Counter comparisons use retired/stopped or otherwise controlled ownership boundaries; individual fields are not an atomic aggregate snapshot. Endpoint capacity is not a whole-runtime memory bound, since delivered buffers can outlive requests.

## Reproduction and audit

```sh
python3 evidence/persistent-read/run.py UNIQUE_NEW_CORRECTNESS_RUN_NAME
python3 evidence/persistent-read/validate.py
```

The runner retains copied sources, manifests/lockfile, runner/oracles, tool/binary hashes and raw command outputs. The [auditor](validate.py) executes no builds, runtime tests or benchmarks; it checks retained/current input hashes, compiler diagnostics, exact public rows, inherited BEAM exit observations and preservation of the preceding milestone inputs. Existing run names are refused.

The pinned BEAM executions remain the inherited thirty-nine-case exit reference. The new persistent-read mapping is supported by a source audit and Rust controls; these are not matched Erlang socket executions. Native errno induction, exhaustive OS scheduling, writes, half-close, endpoint transfer and production performance remain unqualified. Future speed qualification requires another suitable environment and explicit authorization.
