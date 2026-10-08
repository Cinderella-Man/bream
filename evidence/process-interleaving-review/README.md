# Ownership and wakeup interleavings

These are executable **finite sequentially consistent abstractions**, not upstream BEAM runs, generated Rust code, a production scheduler, or a soundness proof. [Report 33](../../docs/research/33-finite-ownership-interleavings.md) explains the source correspondence and exclusions.

Run from the Bream repository root:

```bash
python3 evidence/process-interleaving-review/explore.py
```

[The explorer](explore.py) writes [model-results.json](model-results.json), including its source hash, environment, explored-state counts, a permitted retained-record/freed-body witness, and shortest failure traces for five deliberately broken variants. Both baseline models exhaust their finite reachable state spaces without a modeled counterexample. Broken variants stop at their first failure. The exit code checks these expected research controls, not BEAM or Bream conformance.

Source reference: OTP 29.1.1, `ad05823719d77c8faee87348ea39513d4e2f99c5`. New source reports trace [buffer publication and retirement](../../docs/research/30-buffered-signal-publication-protocol.md), [parking and wakeup](../../docs/research/31-receive-parking-wakeup-protocol.md), and [relationship cancellation](../../docs/research/32-relationship-cancellation-protocol.md). No new upstream VM run or upstream source edit is part of this pass.

The historical validation output is superseded after the user-requested removal of speed-test evidence. The read-only validator entry point now delegates to the [performance-reset audit](../performance-reset/README.md), which checks the surviving file inventory, correctness records and documentation links. Hash inventories describe the filtered files; no runtime or benchmark is executed.