# Rust ownership and deeper process-source review

The twelve fixtures are standalone Rust-language probes. They do not use Bream's process API, exercise preemptive scheduling, benchmark speed or establish runtime soundness. Six fixtures execute, five deliberately fail compilation and one non-returning callback fixture is compiled without execution. Both unoptimized and optimized profiles run: 24 compiler invocations and 12 executable invocations.

Run:

```sh
python3 /home/bat/projects/bream/evidence/rust-ownership-review/run_probes.py
```

The runner uses the existing lab's Rust 1.98.1 compiler directly and read-only, builds separate artifacts beneath Bream's ignored `.tools/rust-ownership-review`, and retains stdout/stderr, expected diagnostic codes, source hashes, binary hashes and commands in `probe-results.json`. Compilation has a 40-second per-command deadline; execution has five seconds. Each subprocess has a separate process group, which is killed on timeout. The probe that never returns is not executed.

`pinned_borrow_cleanup` uses an explicit `await` to inspect Rust-generated storage, not to propose user-authored yielding for Bream. `forget_cleanup` deliberately leaks a tiny allocation until its isolated executable exits; the observed destructor counter is a language-contract counterexample, not a leak in Bream. Pointer equality in `owned_transfer` establishes only that fixture's retained backing allocation, not a performance result.

`prior-case-ids.json` snapshots the pre-pass 423-case catalogue. Reports 38–41 contain the deeper source review and its applicability. The pass's validation checks links, source ranges, case/task indexes and these recorded artifacts; prior reference and model evidence is preserved separately.

Validate current documentation, indexes and recorded probe artifacts without rerunning the probes:

```sh
python3 /home/bat/projects/bream/evidence/rust-ownership-review/validate_evidence.py
```

[Validation result](review-validation.json) checks recorded evidence consistency. It does not prove Rust runtime soundness or complete BEAM coverage.
