# Executed cancellation and peer-boundary observations

**2026-10-09. Correctness only.** The user asked us to research and test this proposed statement:

> I went looking for the hard part of building BEAM in Rust. Preemption wasn't it—we got native code taking turns. The hard part is killing one process without breaking another. Rust's ownership rules don't automatically give you that boundary.

The [independent source audit](58-native-termination-boundary-source-audit.md) and [bounded experiment](../../experiments/termination-boundary/README.md) support the final sentence and the distinction between scheduling and disposal. They do **not** establish the comparative claim about which problem is harder. This report records what actually executed, rather than treating an attractive tweet as a hypothesis that must win.

## Evidence

The final [recorded cohort](../../evidence/termination-boundary/runs/20261009-boundary-03/results.json) has **26 successful commands and 30 expected semantic observations**:

- Nine ordinary Rust cases under each of `opt-level=0` and `opt-level=3`: **18 observations**, compiled with Rust 1.98.1 and `#![forbid(unsafe_code)]`.
- Six Erlang cases on each of the existing OTP29.1.1 release JIT and debug interpreter: **12 observations**. The runner verifies emulator/launcher hashes against the retained build provenance for commit `ad05823719d77c8faee87348ea39513d4e2f99c5`.

There are no benchmarks, speed comparisons or elapsed-time observations. Native preemption was not rerun. Test watchdogs detect hangs; their thresholds are not evidence about responsiveness. Two earlier incomplete harness-development runs are retained and explained in the experiment README. [Read-only validation](../../evidence/termination-boundary/validation.json).

## Ownership does not supply a transactional cancellation boundary

The shared-ledger Rust future satisfies `Send + Sync + 'static`. It subtracts one from the first balance under a mutex, releases the guard, and returns `Pending` before adding one to the second balance. After ordinary future destruction, a peer thread reads **99 + 100 = 199**, despite a starting total of 200. Its mutex is available and unpoisoned. The frame-owned payload is destroyed exactly once. This is a deterministic application invariant failure, **not undefined behavior, corrupted memory, a leaked lock or a defect in Rust ownership**. [Implementation](../../experiments/termination-boundary/src/main.rs).

The completion control yields 200. Moving both changes into a single lock-protected update before the checkpoint also yields 200 after cancellation. An explicit rollback guard restores 200 in the cancelled two-stage example. The rollback control offers compensation for this isolated protocol, not isolation: an observer can see the intermediate 199 and may already have acted on it. No test forcibly terminates a native instruction while a guard is held.

The BEAM control prevents a misleading language comparison. Its surviving host owns an ETS table. A child publishes the first stage, acknowledges that publication, and is then killed before the second stage. **After the matching `Down`, an independent peer reads the same total 199.** Completion and a single complete ledger-object update preserve 200. Ordinary Erlang process death does not roll back a sequence of shared-state operations. That agrees with the source-level distinction between ETS operation atomicity and arbitrary application transactions. [Fixture](../../experiments/termination-boundary/termination_reference.erl), [pinned ETS contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/stdlib/src/ets.erl#L126-L136).

Private-state controls succeed in both environments: cancelling/destroying the Rust future or killing the Erlang process leaves the independent peer's total unchanged. This supports Bream's existing process-owned-state direction; it does not establish general runtime isolation.

## A nominal parent does not own all execution it starts

The Rust parent future starts a native child thread. The harness retains its join handle for cleanup. After the parent future is destroyed and its own payload receipt settles, the harness sends the child a new work command. The child increments the shared counter and acknowledges the effect. An explicit stop and join then release the child's payload. This is new work ordered **after** parent destruction, not work merely queued before cancellation. [Child fixture](../../experiments/termination-boundary/src/main.rs).

An explicit guard that sends stop and joins fixes the bounded cooperative case. It cannot make a noncooperating child safely killable; that guard's join could instead block indefinitely. The test does not drop the untracked child's join handle: detaching on handle drop is an additional documented fact, not an observation from this fixture. [Rust thread ownership](https://doc.rust-lang.org/std/thread/struct.JoinHandle.html).

BEAM's unlinked child also performs new work after its parent's `Down`; a nontrapping linked child instead exits when its parent is killed. Thus parent-child cleanup is policy enforced by relationships, not an automatic consequence of being an actor system. Bream's native-resource boundary must distinguish supported linked process children from independently spawned OS threads.

## Destruction can prevent another task from running

The strongest direct observation about peer progress is the destructor case. A `Send + 'static` future has already returned `Pending`; a worker drops it normally. Its custom destructor announces entry and waits on an empty channel. The worker cannot reach its next peer task until the host releases that channel. After release, destruction completes and the peer returns its expected value. A controlled destructor without that wait lets the same sequence finish directly.

The obstruction follows from channel ordering, not timing or machine load. Ordinary Rust permits this destructor, and memory safety is preserved. The example establishes that scheduling checkpoints in the application body do not govern all work performed during cancellation. Moving cleanup to another thread could protect this worker, but would leave an independently managed cleanup obligation; it would not make the destructor complete without its release condition. [Drop contract](https://doc.rust-lang.org/std/ops/trait.Drop.html), [Future contract](https://doc.rust-lang.org/std/future/trait.Future.html#runtime-characteristics).

This is not a measured cleanup latency, an infinite-destructor execution, or a new finding that the Bream worker currently accepts arbitrary `Drop`. It does not: the checked experiment has a restricted admitted state/type surface. Native NIF/resource cleanup in BEAM has its own exceptions; source review does not support universal immediate BEAM reclamation. [Pinned process/resource exclusions](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L462-L493).

## Consequences for Bream

1. **Maintain separate contracts for execution stop, native/external work, ownership release, and physical cleanup.** A parent frame-drop receipt must not silently claim that child threads or callbacks have stopped.
2. **Keep process-local mutable state as the default.** If shared mutable resources are admitted, their operations need a cancellation policy. Ownership bounds alone do not establish transaction boundaries.
3. **Qualify destructor behavior independently of normal actor execution.** An admitted resource may need explicit staged teardown or independently owned cleanup work. A chunk limit on a mailbox does not bound one arbitrary destructor.
4. **Make escape paths explicit.** Runtime-owned factories, children, registrations and cleanup records can supply structure absent from an unrestricted native closure. An audited operation or instrumented dependency need not imply a permanent whole-crate allowlist.
5. **Continue preemption research separately.** The earlier [arithmetic probe](../../experiments/native-preemption/README.md) demonstrated native scheduling, not arbitrary dependency compatibility. Signals can pause computation; the present experiments do not qualify signal-driven frame destruction.

These findings qualify the accepted Bream design; they neither invalidate the twelve restricted milestones nor complete the next implementation milestone. No production runtime/compiler code changed.

## Public wording

Supported:

> We got native Rust code taking turns. But a task's lifetime isn't the lifetime of everything it starts or touches. Our tests showed child threads outliving it and a destructor blocking the next task. Ownership alone doesn't make an actor safely disposable.

Also say, when comparing with Elixir, that BEAM does not roll back shared effects or automatically kill unlinked children. Avoid claiming that preemption is solved for arbitrary libraries, that C/C++ makes arbitrary native termination safe, or that these bounded counterexamples prove Bream is a bad idea.
