# Process performance source review

This pass adds source research and planned experiments, not runtime performance results. Reports 34–37 examine message storage, scheduler balancing, timer service and checkpoint/continuation costs. The [performance backlog](../../docs/performance-plan.md) and [task index](../../docs/build-plan/performance-tasks.json) distinguish planned experiments from qualified features.

The historical validation output is superseded after the user-requested removal of speed-test evidence. The read-only validator entry point now delegates to the [performance-reset audit](../performance-reset/README.md), which checks the surviving file inventory, correctness records and documentation links. Hash inventories describe the filtered files; no runtime or benchmark is executed.

Run from any working directory:

```sh
python3 /home/bat/projects/bream/evidence/process-performance-review/validate_evidence.py
```