# Room/session correctness evidence

The [ninth milestone](../../experiments/room-process/README.md) integrates the existing checked single-worker primitives into a two-session application. The [runner](run.py) executes correctness controls only. No benchmark, throughput/latency measurement or speed comparison runs; watchdogs detect hangs and incidental Cargo duration text is not analyzed. [Machine constraint](../../AGENTS.md).

Each new cohort copies source and writes its protocol before execution, preserving tool/binary identities, exact commands, raw output and expected results. Both compiler profiles use two build jobs. Existing cohort names are refused.

The final [cohort](runs/20261008-room-01/results.json) passes 317 expected command outcomes and retains 286 structured observations. Each compiler profile includes 83 inherited unit tests, three configurations each of mailbox/timeout/monitor/link/spawn/room controls, 124 expected rejected consumers and seven positive shadowing examples. Runtime/compiler semantics are inherited from the atomic-spawn milestone; this adds application integration coverage.

Six room configurations cover eighteen scenarios: a complete room conversation, forced room death during selective receive with nineteen queued requests, and active host shutdown. The fifteen-record expected transcript covers duplicate join without another monitor, one-target owned routing, unknown/absent members, a session report before failure, monitor-driven membership removal, continued peer communication, timeout before a preserved publish, leave/Down idempotence and orderly room completion. An independent checked CPU loop remains runnable; its logical progress and eventual genuine Down are asserted without measuring speed.

Each room observation includes `expected_transcript` and actual collector `observed_records`, `observed_digest` and `observed_checkpoints`. A checkpoint is `[record_count, cumulative_digest, last_sequence]`. Counts 1–8 and 10–15 are checked at protocol barriers; the timeout and preserved publish form the combined two-record barrier at 10. The digest is wrapping base 131 over eleven words per record. Rows remain labeled expected; the digest is a bounded oracle, not collision-proof proof or a raw capture of every received payload.

The runner and auditor recompute the expected prefixes and check every observed checkpoint. Ownership checks also require one drop and zero live bytes at each original message/input receipt, settled monitors and timers, and no post-stop room execution. Invalid session notifications leave the process alive, so forced room-death completion requires the matching monitor path rather than an error return.

The session-failure control deliberately takes its input a second time after sending its fault report. The worker must contain that panic and satisfy subsequent protocol/ownership assertions. Inherited timeout/link/spawn controls also contain expected failures; those stderr messages are not unexpected cohort failures.

```sh
python3 evidence/room-process/run.py UNIQUE_NEW_CORRECTNESS_RUN_NAME
python3 evidence/room-process/validate.py
```

The [read-only auditor](validate.py) checks retained hashes, current inputs, raw observations, diagnostics and transcript arithmetic. It verifies that runtime/compiler source matches the previous milestone after crate renaming, and checks both earlier spawn cohorts and their final current inputs without rewriting them. It then runs the [performance-reset audit](../performance-reset/README.md). This audit invokes no compiler, runtime fixture, model or benchmark. [Audit result](validation.json).

Limits: two fixed members, one application worker, injected logical time, moved concrete payloads, trusted protocol slots and existing admission restrictions. Broadcast, shared immutable data, dynamic membership, a hogging session and broad fanout/death interleavings remain open. This is partial G5 integration, not a production service or complete BEAM conformance.
