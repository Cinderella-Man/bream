# Process performance source review

This pass adds source research and planned experiments, not runtime performance results. Reports 34–37 examine message storage, scheduler balancing, timer service and checkpoint/continuation costs. The [performance backlog](../../docs/performance-plan.md) and [task index](../../docs/build-plan/performance-tasks.json) distinguish planned experiments from qualified features.

`prior-case-ids.json` preserves the catalogue's 40 groups/392 IDs before this pass. `validate_evidence.py` checks Markdown links and pinned source ranges, feature/task-index agreement, preservation of prior IDs, integration of new report cases and source checkout identity. It also hashes retained earlier model evidence and its validation record; it does not rerun or recertify those models or earlier VM fixtures.

Run from any working directory:

```sh
python3 /home/bat/projects/bream/evidence/process-performance-review/validate_evidence.py
```

The resulting `review-validation.json` is documentation consistency evidence. Source predictions, including repeated same-deadline timer batches and the earlier late-buffer-reinstallation concern, remain unexecuted investigations. There are no new Bream benchmarks, upstream suite results or speedup claims in this directory.
