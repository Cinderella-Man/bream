# Termination-boundary evidence

Correctness only, 2026-10-09. [Method, controls and limitations](../../experiments/termination-boundary/README.md), [source audit](../../docs/research/58-native-termination-boundary-source-audit.md), [findings](../../docs/research/59-termination-boundary-observations.md).

The passing cohort is [20261009-boundary-03](runs/20261009-boundary-03/results.json): 26 successful commands, 18 Rust observations and 12 Erlang observations. Four exact experiment inputs and prior OTP source-build provenance are retained alongside command output and hashes. Compiled artifacts live in ignored `.tools/termination-boundary`.

Development attempts remain transparent: `-01` failed an OTP layout assumption before compilation; `-02` passed the Rust cases and produced correct JIT scenario results but rejected charlist-encoded version metadata. Neither is counted as a passing cohort. The final fixture emits a version string and verifies the actual emulator hashes against the prior pinned build record.

Run the read-only audit:

```sh
python3 evidence/termination-boundary/validate.py
```

It checks source snapshots, outputs, exact result sets, retained binaries and OTP build provenance, and writes [validation.json](validation.json). It does not compile or execute an experiment. The runner's only time-related configuration is external watchdog deadlines; there are no speed observations or comparisons.
