# Historical green-runtime evidence

Correctness only, 2026-10-09. [Method and scope](../../experiments/rust-green-history/README.md), [research conclusions](../../docs/research/63-rust-green-runtime-lessons-and-experiments.md).

The passing cohort is [20261009-green-02](runs/20261009-green-02/results.json): **26 expected command outcomes, 20 observations** across two build profiles. Eight observations execute original Rust 0.12, eight execute modern ownership/lifetime probes, and four are expected compiler rejections. These are eight distinct runtime scenarios plus two rejection controls, each checked under both profiles—not twenty independent designs.

[Provenance](provenance.json) records the official compiler archive URL, published SHA-256 and exact source revision. Selected manifests retain hashes for **164 source/license files** across the 2013 cancellation, Rust 0.12 and late 2014 scheduler snapshots. Original files and compiler artifacts are under ignored `upstream/` and `.tools/`; [prepare.py](../../experiments/rust-green-history/prepare.py) can restore them without installing or executing a toolchain.

Every experiment input is retained in the cohort, with stdout/stderr and hashes. The intentional historical panic emits an expected diagnostic but is caught at task scope. Expected modern compiler failures have their required error codes checked. Failed attempt `-01` used an unsupported historical compiler flag and ran no experiment; it remains visible and is not a passing cohort.

[validation.json](validation.json) is a read-only source/tool/output/expected-result audit:

```sh
python3 evidence/rust-green-history/validate.py
```

It never compiles or executes experiments. There are no throughput, latency, duration or performance-comparison results. The only time-related execution settings are watchdog limits to detect hangs.
