# Ownership and wakeup interleavings

These are executable **finite sequentially consistent abstractions**, not upstream BEAM runs, generated Rust code, a production scheduler, or a soundness proof. [Report 33](../../docs/research/33-finite-ownership-interleavings.md) explains the source correspondence and exclusions.

Run from the Bream repository root:

```bash
python3 evidence/process-interleaving-review/explore.py
```

[The explorer](explore.py) writes [model-results.json](model-results.json), including its source hash, environment, explored-state counts, a permitted retained-record/freed-body witness, and shortest failure traces for five deliberately broken variants. Both baseline models exhaust their finite reachable state spaces without a modeled counterexample. Broken variants stop at their first failure. The exit code checks these expected research controls, not BEAM or Bream conformance.

Source reference: OTP 29.1.1, `ad05823719d77c8faee87348ea39513d4e2f99c5`. New source reports trace [buffer publication and retirement](../../docs/research/30-buffered-signal-publication-protocol.md), [parking and wakeup](../../docs/research/31-receive-parking-wakeup-protocol.md), and [relationship cancellation](../../docs/research/32-relationship-cancellation-protocol.md). No new upstream VM run or upstream source edit is part of this pass.

[Validation](review-validation.json) checks document links/source ranges, the 392-case index, preservation of the [prior 366 IDs](prior-case-ids.json), model hashes/results and retained prior reference evidence. A passing validation means artifact consistency, not runtime correctness. Repeat it with `python3 evidence/process-interleaving-review/validate_evidence.py` when the local source/build artifacts are available.
