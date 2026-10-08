# Monitor and Down correctness evidence

The [sixth milestone](../../experiments/monitor-process/README.md) extends the injected-clock single-worker profile with local monitors and genuine Down notifications. This directory contains correctness evidence only. No speed test, benchmark, throughput or latency measurement is part of the runner. Watchdogs detect hangs; incidental Cargo duration text is not analyzed. [Machine constraint](../../AGENTS.md).

The [runner](run.py) records its protocol and copies source before execution. A fresh cohort name is required. It retains raw outputs, exact compiler and binary identities, source hashes and individual expected outcomes. Both compiler profiles use two build jobs.

The protocol requires 51 internal tests per profile; three mailbox, three timeout and three monitor configurations per profile; 89 rejected consumers per profile; and five positive shadowing examples per profile. Each monitor invocation exercises 16 creation/kill and 16 cancellation/kill races. Race counts describe bounded observed interleavings, never an exhaustive proof. Deterministic tests separately force each cancellation/conversion order, old-incarnation behavior, pending-notification cancellation, bounded relationship cleanup and Down before final tracked reclamation.

The failure-reason control intentionally reads beyond an empty admitted buffer; the worker catches the panic and emits a failed-process Down. Timeout controls retain their two expected invalid-duration/overflow panics. Such stderr is expected only when the control completes with the required terminal state and ownership accounting.

```sh
python3 evidence/monitor-process/run.py UNIQUE_NEW_CORRECTNESS_RUN_NAME
python3 evidence/monitor-process/validate.py
```

The [audit](validate.py) never executes a runtime, model, compiler or benchmark. It verifies retained records, current source identity, raw observations, all command exit codes and compiler diagnostics. The [performance-reset audit](../performance-reset/README.md) preserves the earlier post-removal baseline and checks that no speed-result fields or deleted files have returned. The new milestone adds evidence without rebaselining earlier correctness snapshots.

Both retained cohorts passed 221 expected command outcomes. [01](runs/20261008-monitor-01/results.json) has 50 unit tests per profile; [02](runs/20261008-monitor-02/results.json) adds the actual-worker peer-during-cleanup check, bringing the total to 51. Cohort 02 is the current source baseline. [Validation](validation.json) checks both sets of retained inputs and outputs and compares the final 113 inputs with current source.

Limits: one application worker, injected logical clock, concrete application payload, sealed genuine Down, small reason codes and trusted host allocation/panic behavior. Links, trapping, atomic monitored spawn, arbitrary look-alike notifications, distribution and full G4 qualification remain open. No production latency bound or performance claim follows from these records.
