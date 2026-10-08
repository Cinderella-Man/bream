# Mailbox correctness evidence

The [mailbox milestone](../../experiments/mailbox-process/README.md) passes its declared single-worker profile. This is **correctness-only evidence**. The user's [persistent constraint](../../AGENTS.md) prohibits speed tests and performance comparisons on this machine, which is used concurrently for other work. Historical local timings remain unsuitable performance indicators.

The [protocol](runs/20261008-mailbox-01/protocol.json) was written before retained compilation and execution. The [result](runs/20261008-mailbox-01/results.json) records 155 expected command outcomes, 82 copied inputs, executable/tool hashes and every raw stdout/stderr. Both profiles pass 21 deterministic tests, three consumer-control configurations, 66 expected compilation failures and three shadowing executions. Six public invocations contain 96 send/kill races and 96 concurrent-sender messages, with ownership/order assertions rather than timing summaries.

The [runner](run.py) has no benchmark phase, warmup, affinity override, host-frequency collection or elapsed-time summary. Runtime snapshots contain logical event identifiers and work/ownership counts. Watchdogs detect hangs only. Incidental Cargo duration text in retained build/test logs is not analyzed as performance data. Build parallelism is limited to two jobs.

Run another uniquely named correctness cohort from the repository root:

```sh
python3 evidence/mailbox-process/run.py UNIQUE_NEW_CORRECTNESS_RUN_NAME
```

Existing run names are refused. Rust 1.98.1 and dependencies are pinned; builds use `--offline --locked` and a unique ignored target directory. Development compilation and smoke controls preceded this retained protocol.

Validate existing evidence without executing the runtime:

```sh
python3 evidence/mailbox-process/validate.py
```

The read-only entry point now delegates to the [performance-reset audit](../performance-reset/README.md). The original correctness run is retained, but earlier speed evidence was deleted at user request and historical hash inventories were rebased. Old validation outputs are explicitly superseded. No runtime or benchmark is executed by the audit.
