# Concurrency and clock research evidence

This pass combines source reviews [46](../../docs/research/46-thread-progress-and-retirement-domains.md), [47](../../docs/research/47-process-lock-ownership-and-handoff.md), [48](../../docs/research/48-clock-domains-and-process-deadlines.md) and the separately executed [atomic projections in report49](../../docs/research/49-rust-atomic-protocol-observations.md).

[run_models.py](run_models.py) builds the [standalone model executable](../../experiments/atomic-protocol-models/) with the exact locked dependency graph and Rust 1.98.1. Seven isolated invocations explore three model families: three positive variants complete, and four weakened controls fail at the intended invariant. [model-results.json](model-results.json) records tool/input/binary hashes, build output, builder settings, environment overrides, expected markers, observed counts and paths to [raw output](runs/).

The builder permits two threads, 1,000 branches per execution, no configured preemption/permutation/time pruning and no checkpoint resume. Each child has an external 45-second watchdog; timeout or model branch-limit failure is a failed run, not success. Positive runs reported 6, 4 and 4 closure starts; negative controls stopped after 3, 3, 1 and 1 starts. These are explored executions of tiny projections, not unique state counts or production coverage.

The dependency/toolchain build had a 180-second watchdog and succeeded. Build time and model execution count are not speed benchmarks. Loom cannot model all weak-memory behaviors; kernel calls and code not using its primitives are outside its exploration. No new BEAM execution, Bream implementation or clock fault-injection experiment occurred.

[Prior case IDs](prior-case-ids.json), [prior task IDs](prior-task-ids.json) and [prior evidence hashes](prior-evidence-hashes.json) retain the previous 474-case/70-task state and evidence. [Validation](review-validation.json) checks the new documentation and artifacts without rewriting those historical results.
