# Timed-read correctness evidence

The [sixteenth implementation milestone](../../experiments/timed-read/README.md) admits persistent reads with injected-clock timeouts. The [runner](run.py) contains correctness controls only. Two build jobs and two runtime-test threads limit concurrent validation work. Watchdogs detect hangs; incidental compiler/test duration text is not analyzed. [Machine constraint](../../AGENTS.md).

The [final cohort](runs/20261009-timed-02/results.json) passed on 2026-10-09: 517 expected command outcomes, 462 aggregate observations and 247 copied inputs. Each compiler profile passes 153 runtime tests, 189 rejected consumers and twelve positive shadowing examples. Thirteen public families run at allowances 1, 4 and 32 in both profiles. The six new configurations include 96 data/timeout and 96 kill/timeout races. BEAM contributes twelve new socket and seventy-eight inherited exit observations.

The [audit](validation.json) passes: 1,651 retained hash records across both cohorts, all 247 current inputs matching, and all 233 previous milestone inputs preserved. The [performance-reset audit](../performance-reset/validation.json) also passes.

The [initial failed cohort](runs/20261009-timed-01/results.json) stopped after 41 commands at an inherited exit-fixture assertion: its entire snapshot comparison raced the worker's park-counter update. Waiting for an independent peer to complete before capturing the baseline fixes the fixture's synchronization. The frozen failed inputs/outputs remain available; the passing cohort uses the corrected copied fixture. The timed-read runtime needed no change for this failure.

## Public records

Each `timed_read_controls` output contains seven simple rows:

`[mode, application_result, started, delivered, committed_timeouts, timers_armed, accepted_expiry_callbacks, timers_cancelled]`

| Mode | Scenario | Expected result |
|---|---|---|
| 0 | Zero, empty | Timeout code 700; one attempt, no timer |
| 1 | Zero, buffered [2,3,5] | Data checksum 2020; no timer |
| 2 | Finite, empty | Timeout code 700; one accepted expiry |
| 3 | Finite, data [7,11,13] | Data checksum 7081; timer cancelled |
| 4 | Indefinite, buffered [17,19,23] | Data checksum 16949; no timer |
| 5 | Zero, positive short [41,43] | Data checksum 1314; no timer |
| 6 | Zero, closed peer | EOF code 701; no timer |

The application retains its result/input through a host gate, checks bytes again, and adds its two input elements to the final result. Each scenario settles one request and one buffer. The exact [oracle](oracles.py) also relates input checksums to the socket reference records without equating read-exactly and read-some outcomes.

The sequential case returns four bytes, times out the next read, returns three later bytes, then reaches EOF. An additional cached EOF starts no request. Earlier data remains readable through timeout, reuse and endpoint closure. At the held-pair boundary the checksum is 62627010991; after EOF the application reads both buffers and its original input, producing 21. Request IDs finish at 4.

Its settled row is `[4,4,4,0,1,4,0,1]`, using:

`[started, settled, delivered, discarded, committed_timeouts, buffers_dropped, live_bytes, endpoint_drops]`

Its clock row is `[3,1,2,0]`: three armed, one accepted expiry, two cancelled, none pending. Capacity remains occupied while the endpoint is idle after timeout. Independent peers complete while a finite read waits.

Seven stop rows use:

`[mode, stage, started, delivered, discarded, committed_timeouts, buffers_dropped, endpoint_drops, queued_messages_cleaned, cleanup_batches]`

They cover idle; first read pending; first data held; second read pending; after its timeout; third read pending with old data held; and both data buffers held. Every row settles nineteen queued Words messages in three batches, releases the original input once, leaves no timer and freezes application observations after stop. The [oracle](oracles.py) requires exact rows.

Shutdown retains three simultaneous states with remote peers open: third read pending with earlier data held, first read pending, and idle. Their ownership rows are `[3,3,2,1,1,3,0,1]`, `[1,1,0,1,0,1,0,1]` and `[0,0,0,0,0,0,0,1]`. Scheduler cleanup, driver join and zero pending timers are required.

Each of six configurations includes sixteen data/timeout and sixteen kill/timeout races. Either permitted result is accepted; requests and buffers must settle once. Winner counts are not rates or evidence of scheduling quality.

## Controlled internal and BEAM observations

Seventeen new runtime tests force pending/claimed/ready expiry, skipped attempts, claimed positive/EOF/failure preference, WouldBlock settlement, stop after timeout intent, detached old callbacks, same-slot process reuse and equal mailbox/read timer identities. A twelve-timer control verifies eight then four callbacks per service invocation. Duration/deadline overflow publishes no operation or timer. Dropped futures remove pending registrations. Injected failure and Interrupted-shaped outcomes are explicitly synthetic.

The six-case [BEAM fixture](../../experiments/timed-read/timed_read_reference.erl) runs on pinned release JIT and debug interpreter builds. It asserts zero empty/data, finite timeout then endpoint reuse, infinity, exact stale cancellation and partial-data timeout for read-exactly. It uses no clock reads, elapsed values or latency comparisons. A finite OTP timeout is an API correctness input. Those cases do not recreate every Rust internal interleaving or establish full API equivalence.

The inherited thirty-nine-case exit fixture also runs on both emulator profiles. All preceding Rust control families and compile-boundary controls remain in the cohort.

## Reproduction and integrity

```sh
python3 evidence/timed-read/run.py UNIQUE_NEW_CORRECTNESS_RUN_NAME
python3 evidence/timed-read/validate.py
```

The runner refuses existing run names and retains copied source/configuration, both Erlang fixtures, runner/oracles, tool/binary hashes and raw outputs. The [auditor](validate.py) executes no experiment or build. It checks retained/current hashes, exact semantic rows, compiler diagnostics, both BEAM fixtures and preservation of the preceding milestone inputs.

Clock expiry counts intent; committed stream timeout counts a result that may later be discarded. Atomic counters do not form one instantaneous aggregate snapshot. The endpoint cap does not bound actor-held storage. Timing qualification requires a separate suitable environment and explicit authorization; no performance result is retained here.
