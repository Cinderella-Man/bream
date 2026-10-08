# Process protocol evidence — 2026-10-07

This directory retains targeted observations from OTP 29.1.1, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`, identifying itself as ERTS 17.1. Read [report 29](../../docs/research/29-pinned-reference-observations.md) for interpretation and limits. Reports [26](../../docs/research/26-suspension-system-work-protocol.md), [27](../../docs/research/27-process-lookup-reclamation-protocol.md) and [28](../../docs/research/28-selective-receive-marker-protocol.md) supply the deeper source traces.

## Build and replay

The original `upstream/otp` remains clean. A separate ignored clone, `upstream/otp-reference-build`, supplied the binaries. [Provenance](source-build.json) records source and executable hashes, compiler/platform identity, configuration and build results. The full application build failed in optional wx debugger support; the subsequent tiny/core target and debug interpreter target succeeded. This does not qualify a complete OTP application build.

After recreating those targets, from the Bream repository root:

```bash
python3 evidence/process-protocol-review/run_reference.py --profile jit-1
python3 evidence/process-protocol-review/run_reference.py --profile jit-4
python3 evidence/process-protocol-review/run_reference.py --profile debug-emu-4
```

Replay overwrites that profile's retained outputs. It runs local disposable VMs with core dumps disabled. It does not install Erlang, patch upstream source, exercise distribution or implement Bream.

| Profile | Commands and identities | General observations | Optimized marker result |
|---|---|---|---|
| One-worker release JIT | [Manifest](jit-1/run-manifest.json) | [JSON](jit-1/observations.json) | [Completes](jit-1/priority-marker-normal.log) |
| Four-worker release JIT | [Manifest](jit-4/run-manifest.json) | [JSON](jit-4/observations.json) | [Completes](jit-4/priority-marker-normal.log) |
| Four-worker debug interpreter | [Manifest](debug-emu-4/run-manifest.json) | [JSON](debug-emu-4/observations.json) | [Aborts at the predicted assertion](debug-emu-4/priority-marker-normal.log) |

All three profiles complete 11 general fixture families and the 255/256/257 hibernation boundaries. The six marker checks include **one reproduced failure**, not six passing tests; all three `no_recv_opt` controls complete. Disassembly in each profile verifies marker instructions are present in the optimized candidate and absent in the control. See [combined observations](observations-summary.json).

The runner originally reused compiled-output paths across profiles. The last profile's bytes remained; the earlier two were subsequently recompiled using their exact original commands and working directories. All twelve separately retained artifacts now match the hashes recorded at execution. [Recovery record](compiled-artifact-recovery.json) distinguishes retained final-run bytes from byte-identical reconstructions. Behavioral fixtures were not rerun during that recovery, and original manifests/results were not rewritten.

## Interpretation limits

The timer cohorts check pre-expiry setup and separate read-only from cancellation observations. Notification ordering waits for both notifications to materialize before consuming them. The creator-death fixture does not prove expiry followed death; receiving DOWN first can also occur with an already-queued timeout. The stronger lifetime rule relies on the source trace.

The marker result reproduces an assertion in the specified debug interpreter. Release-JIT controls succeed; this does not demonstrate release message loss or establish behavior on every backend. Source assertions and implementation discrepancies are evidence to investigate, not behavior to reproduce in Bream.

[Review validation](review-validation.json) checks documentation links, case-index consistency, source pins and retained artifact hashes. Its passing status refers to those checks. It does not turn the expected reference abort into a passing conformance case, prove exhaustive coverage or qualify a Bream runtime.

Run `python3 evidence/process-protocol-review/validate_evidence.py` from the repository root to repeat these checks against the local source/build/artifact directories.
