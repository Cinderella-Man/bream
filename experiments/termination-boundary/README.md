# Termination, cleanup and peer integrity

**Correctness research, 2026-10-09.** This experiment tests the claim that native scheduling success does not automatically give Rust tasks a safely disposable process boundary. It does not add a Bream implementation milestone or select a production execution mechanism.

**Result:** all **30 expected semantic observations** passed: nine Rust scenarios in two compiler profiles, and six BEAM scenarios in two VM profiles. [Recorded cohort](../../evidence/termination-boundary/runs/20261009-boundary-03/results.json), [source audit](../../docs/research/58-native-termination-boundary-source-audit.md), [interpretation](../../docs/research/59-termination-boundary-observations.md).

No speed tests, latency/throughput measurements, performance comparisons, elapsed durations or benchmark phases run here. Channels, explicit polls and Erlang messages establish the ordering. External watchdogs detect hangs only. The earlier native interruption probe was **not rerun**.

## Method

The [Rust source](src/main.rs) uses only the standard library and forbids unsafe code. Each command is a fresh executable process. The harness pins an ordinary future, polls it until an explicit test gate returns `Pending`, then either polls again or drops it through ordinary Rust destruction. It never frees an executing frame or injects asynchronous unwinding. Explicit awaits are instrumentation for the experiment, not proposed Bream application syntax.

Owned-state, shared-update and rollback futures are checked at compilation against `Send + Sync + 'static`. Child-thread and gated-destructor futures satisfy `Send + 'static`. These are ordinary Rust admission counterexamples; the current Bream macro does not admit this unrestricted code.

Rust uses the retained 1.98.1 compiler with `opt-level=0` and `opt-level=3`, `panic=unwind` and warnings denied. These profiles check semantics under optimization, never speed. The fixtures contain no panic on their expected path. `frame_drops` counts the destructor of the frame-owned instrumented payload; it is not a count of all allocations, frames or physical OS reclamation.

The [Erlang source](termination_reference.erl) runs on the existing OTP29.1.1 release JIT and debug interpreter, with one normal scheduler and one scheduler for each dirty class. Each VM executes six finite scenarios and exits. It does not run a NIF. The runner verifies both source checkout commits and matches the existing emulator/launcher hashes against the retained [source-build provenance](../../evidence/process-protocol-review/source-build.json), then saves that provenance with the run. No OTP build is performed.

## Rust observations and controls

| Scenario | Observed result | Scope of conclusion |
|---|---|---|
| `owned_cancel` | The target's private partial ledger is discarded; its payload drops once; independent peer total stays 200. | Ownership plus a qualified execution/cleanup boundary can preserve peer state. |
| `shared_cancel` | A two-stage transfer is cancelled after debit; a peer thread reads 99 + 100 = 199 after frame destruction. The mutex is available and unpoisoned; payload drops once. | Safe ownership/sharing and normal cancellation do not roll back an application protocol. There is no memory-safety failure. |
| `shared_complete` | Resuming the same transfer performs credit; peer reads 99 + 101 = 200. | The incomplete protocol, rather than its arithmetic, accounts for the lost invariant. |
| `atomic_commit_cancel` | Both changes occur under one lock before the checkpoint; cancellation preserves total 200. | Publishing the complete change before this cancellation boundary avoids the demonstrated failure. It does not qualify arbitrary-PC cancellation while holding that lock. |
| `rollback_cancel` | A purpose-built guard restores total 200 when cancelled; one rollback runs. | Explicit compensation can restore this protocol. The host observes total 199 before cancellation: this is **not** isolation or rollback of effects already observed elsewhere. |
| `child_survives` | After the parent future drops, a native child receives a new work command and increments a shared counter. An explicit stop and join then release its payload. | Dropping the nominal parent does not automatically stop independently spawned execution. |
| `managed_child_cancel` | A guard sends stop and joins an idle, cooperative child; no child effect occurs, resources drop, and later work is rejected. | Explicit lifecycle policy solves this bounded case. Joining arbitrary noncooperating work can itself block; this is not a general kill primitive. |
| `blocking_drop` | The destructor reports entry, then waits on a channel. Cleanup remains unfinished and the next peer on that thread cannot run until the host releases the gate. Afterwards both finish. | Even normal, memory-safe destruction can obstruct a worker independently of application scheduling checkpoints. No duration is inferred. |
| `bounded_drop` | A controlled destructor finishes and the next peer replies on the same cleanup thread. | The peer fixture is serviceable when cleanup has no external wait. |

The native child fixture deliberately hands its join handle to the harness for guaranteed teardown. It demonstrates surviving execution, not experimentally dropping that join handle. The standard-library detach-on-handle-drop rule is a separate source fact. Every spawned Rust thread is joined before its command completes; no infinite computation, orphan worker or forced native cancellation is used.

## BEAM controls

| Scenario | Observed result | Implication |
|---|---|---|
| `owned_kill` | A killed process's partial private state does not change an independent peer's total 200. | The ordinary process boundary provides private-state containment in this fixture. |
| `ets_cancel` | After `kill` and the matching `Down`, the surviving peer reads total 199 in a host-owned public ETS table. | Ordinary BEAM process death also does not roll back a multi-operation shared protocol. |
| `ets_complete` | Completing the second update yields total 200. | Matched completion control. |
| `ets_atomic_cancel` | Publishing one complete ledger tuple before death leaves total 200. | A complete single-object update avoids this partially published state. |
| `unlinked_child` | An unlinked child handles new work after its parent's `Down`. | Child termination is an explicit relationship/policy decision in BEAM too. |
| `linked_child` | Parent kill causes the nontrapping linked child to exit with `killed`. | A runtime-maintained lifecycle relationship supplies the missing policy. |

These checks are intentionally comparable semantic examples, not equivalent Rust/BEAM APIs or a contest between implementations. The destructor fixture has no directly corresponding ordinary Erlang user `Drop`; native NIF resources require their own separate analysis.

## Reproduce and audit

Run only this correctness runner with a new label:

```sh
python3 experiments/termination-boundary/run.py --label your-unique-label
```

It snapshots the four experiment inputs, compiles isolated artifacts under ignored `.tools/termination-boundary`, records commands/stdout/stderr and hashes, validates exact JSON oracles, and preserves failures. The reference toolchain/build paths are explicit in [run.py](run.py). Ordinary Cargo can also build the dependency-free Rust source, but the recorded cohort used rustc directly.

Audit without executing experiments:

```sh
python3 evidence/termination-boundary/validate.py
```

The final cohort contains 26 successful commands and 30 expected observations. Earlier development runs remain: `20261009-boundary-01` stopped before compiling because the runner assumed an installed OTP directory layout; `20261009-boundary-02` passed all 18 Rust observations and produced the six expected JIT results, then rejected OTP's release charlist representation while checking metadata. The final fixture encodes that metadata as a JSON string, and the runner verifies the actual build provenance. Neither earlier result is reported as a passing cohort.

## What remains unresolved

We have not qualified arbitrary library interruption, cancellation at arbitrary native instructions, cleanup deadlines, worker migration, resource-registration escape or the complete current Bream admission boundary. The earlier arithmetic preemption experiment remains narrow. These tests do not prove termination is intrinsically harder than preemption, that C/C++ avoids the issues, that Rust requires a crate allowlist, or that Bream cannot offer broad useful compatibility.
