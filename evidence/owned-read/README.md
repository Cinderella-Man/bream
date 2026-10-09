# Owned-read correctness evidence

The [fourteenth milestone](../../experiments/owned-read/README.md) adds one runtime-owned nonblocking local socket read per actor. The [runner](run.py) contains correctness controls only: no throughput/latency measurements, speed tests or performance comparisons. Watchdogs detect hangs; incidental compiler/test duration text is not performance evidence. Builds and Rust tests use two concurrent jobs/threads. [Machine constraint](../../AGENTS.md).

The [passing cohort](runs/20261009-read-01/results.json) retains **454 expected command outcomes, 408 aggregate observations and 219 inputs**. Debug and release each pass 119 runtime tests, 169 expected compiler rejections and ten positive shadowing executions. Eleven public control families run at allowances 1, 4 and 32, including six new read configurations and all inherited families. The inherited pinned BEAM exit fixture passes thirty-nine observations on both its JIT and debug emulator.

The [read-only audit](validation.json) verifies 1,157 hash records, every current input and all 205 preceding-milestone inputs. The [speed-evidence removal audit](../performance-reset/validation.json) also passes. Aggregate observation counts contain each public configuration/reference output as one record; they are not counts of every assertion or a state-space coverage claim.

## New semantic observations

Each `read_controls` record contains two completion rows, `[terminal_class, result, repeated_read_unavailable_count, live_read_bytes, storage_drops, endpoint_drops]`. Both must be `[1,62629,1,0,1,1]`: checksum62627 from bytes `[2,3,5,7]` plus the two owned Words. One begins with a pending read, the other with a completed result held before application consumption. The helper waits while owning returned bytes, reads them again and ends normally. A separate unit check requires the Vec backing address to remain unchanged across driver-to-actor transfer.

The three stop rows are `[mode, terminal_class, final_application_stage, delivered, queued_messages_cleaned, cleanup_batches, storage_drops, endpoint_drops]`:

| Stop boundary | Expected row |
|---|---|
| Actor waiting for socket data | `[0,2,2,0,19,3,1,1]` |
| Driver completed, actor has not consumed | `[1,2,1,0,19,3,1,1]` |
| Actor/helper owns the returned bytes | `[2,2,3,1,19,3,1,1]` |

Each checks exactly-once Words/read-buffer release, nineteen queued Words messages cleaned in three batches, and frozen application observations after stop. Endpoint and buffer counters describe their admitted owners; they are not RSS or a total memory bound. Exact counter comparisons use established ownership boundaries.

Other public assertions distinguish EOF0 from unavailable90, complete an independent checked peer while a read is pending, reject capacity exhausted by a ready result, reuse that capacity after disposal, isolate an old PID from a same-slot replacement, and settle two pending reads at runtime shutdown with remote endpoints still open. Six configurations each include sixteen concurrent write/kill cases. `race_deliveries` may range from 0 to16; no distribution, elapsed time or winner rate is interpreted as performance evidence.

## Internal phase and fault controls

Fourteen new runtime tests cover real short reads/HUP, EOF/WouldBlock, buffer transfer, pending/ready admission, consumption uniqueness, size and origin errors, and shutdown. Controlled phases keep a claimed request alive after actor cleanup and registry-slot reuse, then complete it without touching the replacement. A duplicate injected completion drops its own payload while preserving the original result. A synthetic failed result verifies the typed branch and disposal; it does not induce a native read error. Dropping the private control writer injects driver wake failure and verifies EOF-driven closure and waiter wakeup.

The application wait never owns the pending buffer. Cancellation can suppress a read before its final check; a claimed read that passed that check may consume bytes after actor death. Late disposal is allowed; delivery to a replacement is not. The source contract and implementation explicitly distinguish synchronous readiness-based reads from kernel-submitted asynchronous operations.

## Reproduction

```sh
python3 evidence/owned-read/run.py UNIQUE_NEW_CORRECTNESS_RUN_NAME
python3 evidence/owned-read/validate.py
```

The runner freezes Rust/manifests/lockfile, runner/oracles, inherited Erlang exit fixture and existing OTP build provenance before execution. It retains exact commands, tool/binary hashes and raw outputs. Existing cohort names are refused. The [auditor](validate.py) performs no build, runtime experiment or benchmark. It verifies current/frozen inputs, command statuses, compiler diagnostics, exact public read/exit rows, inherited BEAM reference agreement and preservation of the preceding milestone's inputs.

Read references are a source audit against pinned OTP/Rust and Linux documentation. The retained BEAM executions exercise the inherited thirty-nine-case exit fixture; they are not new socket/read equivalence tests. Future speed qualification requires another suitable environment and explicit authorization.
