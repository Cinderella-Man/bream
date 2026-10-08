# Native Rust preemption mechanism probe

Executed 2026-10-07 on Linux x86_64 with Rust 1.98.1. **Two plain native Rust computations made overlapping progress on one task-worker OS thread, with no application yield or await.** This uses published `open-coroutine-core = 0.7.0`, not Wasm. It establishes that the native preemption mechanism is achievable here. It does not establish that arbitrary Rust programs and native libraries can safely compose with it.

The final source is [src/main.rs](src/main.rs), dependencies are fixed by [Cargo.lock](Cargo.lock), and execution is bounded by the separate Python process in [run.py](run.py). No original lab source or toolchain installation was modified. Downloaded dependency caches, build products and the standalone rustfmt component remain under this experiment's ignored directories.

## What is exercised

One `Scheduler` runs on the main OS thread. It receives two closures. Each performs 100 million arithmetic iterations and periodically records progress through atomics. Neither computation calls `yield`, awaits a future, allocates, takes a lock, performs I/O, sleeps, reads time or makes a syscall inside the loop. Atomics observe execution; they do not notify the scheduler. Both must observe the other's progress before that other computation finishes.

The runtime itself starts a monitor helper thread and sends `SIGURG` to the task worker when its monitored execution exceeds a quantum. Its signal handler suspends the current coroutine; the scheduler can run another. The monitor does not execute either computation. Therefore **one task worker** is accurate; “the whole program has one OS thread” would be false. See the bundled, unmodified published source: [monitor startup and signal handler](vendor/open-coroutine-core-0.7.0/src/monitor.rs), [10 ms monitor listener](vendor/open-coroutine-core-0.7.0/src/monitor.rs), and [scheduler implementation](vendor/open-coroutine-core-0.7.0/src/scheduler.rs). Published package: [open-coroutine-core 0.7.0](https://docs.rs/crate/open-coroutine-core/0.7.0/source/src/monitor.rs).

The Linux task ID is sampled immediately before and after each computation, outside its arithmetic loop; both values must match the scheduling thread. Runtime source plus this direct single-scheduler arrangement establish the one-worker scope. This is not a trace of every machine instruction or CPU affinity: the OS may move that one thread between CPU cores.

## Final observations

| Check | Preemption enabled | Same source, preemption disabled |
|---|---:|---:|
| Worker task ID | 91111 | 91114 |
| Computation start task IDs | 91111, 91111 | 91114, 91114 |
| Computation finish task IDs | 91111, 91111 | 91114, 91114 |
| Both computations finished | Yes | Yes |
| Both observed the other while still running | Yes | No |
| Sampled dispatch changes, including first dispatch | 26 | 2 |
| Observed task-to-task changes after subtracting first dispatch | 25 | 1 |
| Probe exit code | 0 | 1, expected negative result |
| External runner verdict | As expected | As expected |

Both executions produced identical final checksums: `8956703282129858646` and `17801881488120503101`. The no-preemption build finishes both computations serially, so merely seeing both finish is insufficient; the overlap condition and repeated changes are the differentiating evidence. This also checks that preemption did not change the result of these two arithmetic calculations. [Final enabled result](../../evidence/native-preemption/bream-preemptive/result.json), [negative-control result](../../evidence/native-preemption/bream-control/result.json), [comparison and hash verification](../../evidence/native-preemption/comparison.json).

`sampled_dispatch_changes` records a change in the most recently observed computation ID, sampled once per 16,384 iterations. It includes the initial idle-to-task transition. It is not an exact count of scheduler context switches: a task can be resumed briefly without reaching the next observation. Final enabled execution took about 263 ms and control about 223 ms, but these single executions are **not an overhead benchmark**. Prior development observations differed substantially under concurrent compilation.

Every final result records binary, manifest, lockfile and source hashes. The two final binaries remain in separate target directories; hashes were recomputed after both runs and match. The [historical archive](../../evidence/archive/pre-bream/README.md) retains the original source, executables, and development/final observations from before the rename. Use `bream-preemptive` and `bream-control` for the current source.

## Reproduce

With ordinary Cargo/Rust 1.98.1 available, run from this directory. Both runs are bounded by the runner's 10-second external deadline; it kills the child on timeout and preserves output. Core dumps are disabled for the child. Labels must be fresh.

```sh
cargo build --release --locked -j3
python3 run.py --label replay-preemptive --binary target/release/bream-native-preemption-probe --expect-preemption yes
cargo build --release --locked --no-default-features --target-dir target/no-preemptive -j3
python3 run.py --label replay-control --binary target/no-preemptive/release/bream-native-preemption-probe --expect-preemption no
```

To reuse this machine's lab toolchain without modifying that lab, set these variables for the command session:

```sh
export CARGO_HOME=/home/bat/projects/bream/experiments/native-preemption/.cargo-home
export RUSTUP_HOME=/home/bat/projects/rust-actor-runtime-lab/.tools/rustup
export PATH=/home/bat/projects/rust-actor-runtime-lab/.tools/cargo/bin:$PATH
```

The initial minimal dependency selection (`korosensei` plus `preemptive`, defaults off) failed because published core source references the optional `crossbeam-utils` dependency without those features enabling it. The final manifest explicitly enables that existing dependency feature. No upstream source was patched. [Original build failure](../../evidence/archive/pre-bream/native-preemption/build-preemptive.log), [successful corrected build](../../evidence/archive/pre-bream/native-preemption/build-preemptive-fixed-features.log), [final enabled build](../../evidence/native-preemption/build-preemptive-final.log), [final control build](../../evidence/native-preemption/build-no-preemptive-final.log).

`cargo fmt` was unavailable in the lab toolchain. The matching rustfmt 1.98.1 component was downloaded from Rust's distribution manifest into this experiment, its archive hash verified against that manifest, and invoked with the existing toolchain's shared libraries. Final `rustfmt --check` passes.

## What this does not establish

The full `open-coroutine` hook integration is deliberately absent from this restricted arithmetic probe. Its own bundled [coroutine documentation](vendor/open-coroutine-core-0.7.0/docs/en/coroutine.md) recommends the public crate with hooks to maintain syscall state; [monitor documentation](vendor/open-coroutine-core-0.7.0/docs/en/monitor.md) specifically directs core users to read the hook requirements. This experiment is not a suggested production embedding recipe.

In particular, it does not test signal-handler reentrancy, allocation during interruption, synchronization with an interrupted lock holder, thread-local state interactions, arbitrary foreign functions, signal conflicts with another runtime, cancellation/destruction of unfinished stacks, panic handling, Rust aliasing invariants or migration between workers. The success of this narrow program proves none of those safety properties. No manual signal handler or context-switch assembly was invented for this project; those mechanisms are entirely the existing dependency's implementation.

There is no process isolation, actor mailbox, supervision, bounded memory accounting or “tiny process” density evidence here. This is the **native scheduling mechanism gate**. A production library still needs a defensible compatibility and safety contract before building general process semantics on top.

Published crate archives, their declared Apache-2.0 license metadata and VCS metadata are retained in `vendor/`; these archives do not include standalone license text files. [Environment and archive hashes](../../evidence/native-preemption/environment.json) identify this as registry 0.7.0; it must not be conflated with the current GitHub development branch. An attempt to retrieve a separate historical hook overview at the crate-recorded commit returned 404; the actual bundled package documentation is the source cited here.
