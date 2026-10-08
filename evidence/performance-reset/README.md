# Performance evidence removed — all speed tests require new runs

On 2026-10-08 the user requested removal of all speed-test evidence and that the tests be redone. This supersedes earlier instructions to preserve historical timing results. No speed test, benchmark or runtime test was executed during this cleanup. The machine remains unavailable for performance qualification.

Dedicated static-payload and worker-notification timing runs were deleted, including raw samples, warmups, summaries and comparison verdicts. Measurement commands and observations were removed from mixed useful-process, structured-process and identity/lifecycle cohorts. Recorded elapsed times were stripped from other mixed correctness records and raw JSON streams. Timing-based claims were removed from reports and navigation. The pre-rename archive now contains source/configuration only, without reports, results or executables.

Implementation source, benchmark source/protocols for redesign, compilation controls, finite models, ownership observations and the mailbox/receive-timeout correctness cohorts remain. Filtered mixed results carry an explicit revision marker; they are not claimed to be unchanged original logs or fresh executions. Watchdog settings and injected logical time are correctness inputs, not speed measurements. Incidental Cargo build/test duration text is not performance evidence and is not analyzed.

[Removal receipt](removal.json) lists deleted/filtered files without retaining their speed values. Historical validation outputs are explicitly superseded. Surviving hash inventories have been rebased; the [new integrity manifest](manifest.json) and [audit result](validation.json) define the post-removal baseline. Older read-only validator entry points delegate to this audit. It never executes experiment runners or binaries.

Later correctness milestones may add evidence without rewriting this baseline. The audit verifies every baseline file and scans subsequent records for forbidden speed-result fields; each new milestone supplies its own input/output integrity checks.

```sh
python3 evidence/performance-reset/validate.py
```

Performance work requiring new qualification includes payload representation, notification policies, scheduling allowances, peer/stop/cleanup latency, registry overhead, process churn and borrowed-versus-retained lookup. The [107-task backlog](../../docs/performance-plan.md) remains planned, with no retained speed result. Reruns require another suitable environment and explicit authorization. Do not run the preserved combined benchmark runners on this machine or restore their old outputs.
