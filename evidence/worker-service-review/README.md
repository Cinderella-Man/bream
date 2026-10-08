# Worker and service research validation

This pass integrates reports [54](../../docs/research/54-worker-sleep-wake-and-controller-boundaries.md)–[57](../../docs/research/57-worker-notification-models-and-load-observations.md), the user's responsiveness-first experiment priority, 14 additional branch cases and eight performance tasks. The catalogue now has 537 cases and 107 tasks. No production runtime feature is qualified.

Run the read-only evidence checks from the repository root:

```sh
python3 evidence/worker-service-review/validate_evidence.py
```

The historical validation output is superseded after the user-requested removal of speed-test evidence. The read-only validator entry point now delegates to the [performance-reset audit](../performance-reset/README.md), which checks the surviving file inventory, correctness records and documentation links. Hash inventories describe the filtered files; no runtime or benchmark is executed.

The worker notification evidence now retains only four finite Loom controls. All worker/payload timing stages and derived comparisons were deleted at the user’s request. [Removal record and replacement audit](../performance-reset/README.md).