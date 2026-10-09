# Explicit exit correctness evidence

The [thirteenth implementation milestone](../../experiments/explicit-exit/README.md) adds checked self-exit and targeted local exit for five reason values. The [runner](run.py) performs correctness checks only. No speed tests, sampled throughput/latency or performance comparisons run; watchdogs only detect hangs. Incidental build/test duration text is not used as evidence. [Machine constraint](../../AGENTS.md).

The [passing cohort](runs/20261009-exit-01/results.json) retains **424 expected command outcomes, 382 aggregate observations and 205 inputs** copied before execution. Tool/binary identities, exact commands and raw output are retained. Builds use two jobs; existing cohort names are refused. Runtime/compiler sources from prior milestones are preserved separately.

## Coverage and observations

Debug and release each include 105 runtime tests, 159 expected compiler rejections and nine positive shadowing executions. Ten public control families run at allowances 1, 4 and 32: inherited mailbox, timeout, monitor, link, spawn, room, sharing, busy-member and replacement controls, plus the new exit controls. These settings vary logical checkpoint boundaries, not measured scheduling quality.

Each of the six exit configurations emits thirty matrix rows with columns `[mode, reason, trap, outcome, observed_reason, returned]`:

- Mode 1 is uncaught self-exit, 2 is legacy self-targeted exit, 3 is peer-targeted exit.
- Reasons 1–5 are Normal, Killed, Failure, NoProcess and Kill. Trap is 0 or 1.
- Outcome 0 means ignored, 1 means an Exit message, 2 means target termination.
- `returned` describes the operation's caller. It is 1 for every peer-targeted case even when the target dies. It is 0 for uncaught self-exit and fatal self-targets.

Rows are derived from actual application/terminal observations after assertions. Expected rows are generated independently in [oracles.py](oracles.py). The cleanup rows are `[mode, reason, trap, final_stage, words_released, queued_messages_cleaned, cleanup_batches, references_created, references_dropped, physical_drops]`. Self cases queue nineteen shared envelopes and require three cleanup batches, twenty-one references released and one final backing drop. Peer cases require two references and one backing drop. A surviving host checks the bytes before final release.

Each configuration also requires two computing-target rows: `[requested_reason, terminal_class, terminal_reason, final_stage, words_released, queued_messages_cleaned, cleanup_batches]`. Expected values are `[3,3,3,10,1,19,3]` and `[5,2,2,10,1,19,3]`. The target has no further receive; application observations remain frozen after stop, and independent peers complete before and after it. The stale-target control forces slot reuse, invokes checked Kill against the old full PID, and requires no change in the replacement.

Runtime tests separately cover exact Down/reference identity, link preservation and propagation, same-sender order, current trapping state, missing/stale/foreign targets, first-fatal arbitration, failed-link non-overwrite and sixteen host-kill/direct-exit races per profile. Race assertions allow either legitimate winner while requiring one final reason and one notification.

## BEAM reference

The pinned source is OTP-29.1.1 commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. The existing compiler/JIT/debug-emulator hashes are verified against [build provenance](../process-protocol-review/source-build.json). Erlang uses one scheduler and one scheduler of each dirty kind.

The [reference fixture](../../experiments/explicit-exit/exit_reference.erl) runs thirty matrix cases, four three-process propagation cases, two catch distinctions, one dead-target case covering all five reasons, and two ordering cases. Both JIT and debug emulator match the thirty-nine expected rows: **78 reference observations**. All six Rust matrices match both reference matrices. BEAM catch behavior is a contrast documenting an excluded capability; the checked Rust profile models uncaught self-exit only.

The 382 aggregate observations contain each emulator's reference output as one record and each public configuration as one record; they are not a count of every assertion or an exhaustive state-space claim.

## Reproduction and audit

```sh
python3 evidence/explicit-exit/run.py UNIQUE_NEW_CORRECTNESS_RUN_NAME
python3 evidence/explicit-exit/validate.py
```

The [auditor](validate.py) runs no builds or experiments. It verifies 1,081 hash records, all 205 current inputs, all 187 preceding milestone inputs, expected command statuses/diagnostics, positive shadow outputs, exact new semantic/ownership rows and reference agreement. [Passing audit](validation.json). The separate [performance-removal audit](../performance-reset/validation.json) also passes; removed speed evidence remains absent.

The implementation uses eager local direct-signal receipt. It does not claim OTP backlog interleavings, arbitrary reason/catch support, arbitrary native cancellation or multiple-worker safety. Future performance qualification requires another suitable environment and explicit authorization.
