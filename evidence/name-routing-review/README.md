# Name-routing reference and variance evidence

[Report52](../../docs/research/52-name-resolution-and-typed-routing.md) traces the source and Rust implications; [report53](../../docs/research/53-name-routing-reference-and-variance-observations.md) interprets the observations and limits.

Run from the repository root:

```bash
python3 evidence/name-routing-review/run_probes.py
```

Replay overwrites only this probe's manifest, raw streams, compiled fixtures and ignored Rust executables. It requires the existing pinned OTP core build and Rust 1.98.1 paths recorded by [run_probes.py](run_probes.py). It performs no install, upstream modification, distributed operation or Bream production build.

[probe-results.json](probe-results.json) records source/binary identity, command arguments, working directory, environment overrides, watchdogs, expected observations and artifact hashes. Every VM profile retains its own compiled fixture under `runs/`; successful Rust executables are in the ignored `.tools/name-routing-review/` directory. Failed commands/timeouts cannot count as a positive control, and expected compile rejection requires its diagnostic.

Eight BEAM families completed in three VM profiles, including 300 total race rounds. Four Rust compilation attempts produced two executable covariance counterexamples and two expected invariant-conversion rejections. These are semantic/type-system observations, not timing benchmarks or full conformance evidence. Command durations are harness bookkeeping only. Earlier evidence remains untouched.
