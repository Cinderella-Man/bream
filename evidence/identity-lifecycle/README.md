# identity-lifecycle correctness evidence

**Performance evidence removed, 2026-10-08, at the user’s request. All speed tests must be redone in an authorized suitable environment. No speed tests may run on this machine.**

The [filtered cohort](runs/20261008-identity-01/results.json) retains 145 correctness/build commands. Eleven unit tests per profile, eighteen public controls, 114 expected compilation rejections and four shadowing executions. Source snapshots and applicable compiled artifacts remain. Raw speed-test commands/outputs, sampled timings and all derived comparisons were removed. Mixed records carry an explicit revision marker; they are filtered historical observations, not newly executed or byte-identical original logs.

The original experiment and runner sources remain for future redesign. These older combined runners contain measurement phases and must not run on this machine. Use the latest correctness-only milestone runners for ongoing implementation.

Read-only validation now uses the [performance-reset audit](../performance-reset/README.md). Its manifest verifies the surviving evidence and documents the new integrity baseline. Older validation outputs are explicitly superseded; they no longer claim that the deleted timing files are present.

```sh
python3 evidence/performance-reset/validate.py
```
