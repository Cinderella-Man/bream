# Rust ownership and deeper process-source review

The twelve fixtures are standalone Rust-language probes. They do not use Bream's process API, exercise preemptive scheduling, benchmark speed or establish runtime soundness. Six fixtures execute, five deliberately fail compilation and one non-returning callback fixture is compiled without execution. Both unoptimized and optimized profiles run: 24 compiler invocations and 12 executable invocations.

Run:

```sh
python3 /home/bat/projects/bream/evidence/rust-ownership-review/run_probes.py
```

The runner uses the existing lab's Rust 1.98.1 compiler directly and read-only, builds separate artifacts beneath Bream's ignored `.tools/rust-ownership-review`, and retains stdout/stderr, expected diagnostic codes, source hashes, binary hashes and commands in `probe-results.json`. Compilation has a 40-second per-command deadline; execution has five seconds. Each subprocess has a separate process group, which is killed on timeout. The probe that never returns is not executed.

`pinned_borrow_cleanup` uses an explicit `await` to inspect Rust-generated storage, not to propose user-authored yielding for Bream. `forget_cleanup` deliberately leaks a tiny allocation until its isolated executable exits; the observed destructor counter is a language-contract counterexample, not a leak in Bream. Pointer equality in `owned_transfer` establishes only that fixture's retained backing allocation, not a performance result.

The historical validation output is superseded after the user-requested removal of speed-test evidence. The read-only validator entry point now delegates to the [performance-reset audit](../performance-reset/README.md), which checks the surviving file inventory, correctness records and documentation links. Hash inventories describe the filtered files; no runtime or benchmark is executed.

Validate current documentation, indexes and recorded probe artifacts without rerunning the probes:

```sh
python3 /home/bat/projects/bream/evidence/rust-ownership-review/validate_evidence.py
```