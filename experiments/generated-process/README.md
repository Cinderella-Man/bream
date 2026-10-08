# Generated native process research subset

Executed 2026-10-07 on Linux x86_64, Rust 1.98.1. **Ordinary Cargo and an attribute proc macro can turn non-yielding process source into native code with mandatory suspension points, allowing the runtime to drop an infinite process normally after control returns.** This executable demonstrates that mechanism for a small, closed source language. It does not accept general Rust programs.

The author writes no `.await`, explicit yield, runtime checkpoint, or async function:

```rust
#[process]
fn infinite_process() {
    let _owned = owned_buffer(4096);
    let mut counter: u64 = 0;
    loop {
        counter += 1;
        observe(counter);
        continue;
    }
}
```

`owned_buffer` and `observe` are recognized syntax in this research subset. They are lowered to approved runtime operations, rather than resolved as arbitrary author-supplied functions. The [macro](macros/src/lib.rs) validates and reconstructs the whole body, adds a mandatory checkpoint at every loop head, and emits an ordinary Rust async state machine. [Authored processes and private executor](runtime/src/main.rs), [trusted runtime builtins](runtime/src/lib.rs).

## Observed result

Final evidence: [bream-final/result.json](../../evidence/generated-process/bream-final/result.json), [raw execution output](../../evidence/generated-process/bream-final/execution.stdout), [final hash verification](../../evidence/generated-process/bream-final/verification.json).

| Observation | Result |
|---|---:|
| Executor task workers | 1, `ThreadId(1)` |
| Polls of infinite process before cancellation | 21 |
| Infinite process progress at cancellation | 20 iterations |
| Survivor progress when cancellation happened | 14 |
| Survivor final progress | 64 |
| Tracked infinite-process buffer payload | 4,096 bytes |
| Buffer destructors observed | 1 |
| Buffer destructors while a poll was active | 0 |
| Tracked live payload after future drop returned | 0 bytes |
| Pure `loop {}` polls returning `Pending` | 2 of 2 |
| Rejected unsupported-source fixtures | 10 of 10 |

The pure empty-loop process contains no builtins at all. Two independent polls each return `Pending`, then its future is dropped outside a poll. This directly rules out the progress-observation builtin as the source of suspension.

The main scenario polls two generated futures in a simple round-robin loop. The infinite process holds its buffer across checkpoints, and the survivor advances between those checkpoints. Once the first has made 20 iterations, the executor verifies its poll returned `Pending`, verifies the payload is still live, removes the pinned future from its slot and calls ordinary Rust `drop`. The compiler-generated future destructor drops its live `OwnedBuffer`, whose `Vec` storage is then destroyed normally. The survivor continues to completion.

This uses standard `Future`, `Pin`, `Wake` and `Waker`, without Tokio, another worker, timer thread, signal handler, custom compiler, context-switch assembly, or unsafe cancellation. Both runtime source files forbid unsafe code. The proc macro uses standard stable Rust parsing/code generation dependencies.

“Forced termination” here means the runtime can discard the process when the generated checkpoint returns control, without an authored cancellation branch. It does **not** mean interrupting arbitrary native instructions at any instant. The runtime never destroys a future while its `poll` call is executing.

## Closed grammar and exact semantics

The checker is intentionally conservative:

| Accepted source | Limits |
|---|---|
| Process declaration | Private synchronous nongeneric `fn name()` with implicit unit result; no arguments or other attributes. |
| Primitive locals | Initialized simple bindings of `u64` or `bool`, optionally mutable; no shadowing, destructuring, references or raw identifiers. Type annotations are rewritten as absolute primitive paths. |
| Values | Primitive literals and declared primitive locals; parentheses, wrapping integer arithmetic, integer bit operations, primitive comparisons, boolean logic and unary `!`. |
| Integer arithmetic | `+`, `-`, `*` and `+=` become wrapping `u64` operations. This intentionally differs from ordinary debug-build overflow panic semantics. Division, remainder and shifts are rejected. |
| Statements | Primitive assignment, approved `observe(u64)`, nested blocks, `if`/`else`, label-free `break`/`continue`, `loop`, `while`, and exclusive `u64` range `for`. |
| Owned payload | `let name = owned_buffer(u64)` creates one opaque runtime buffer. The author cannot read, move, borrow, mutate, alias, pass, or explicitly destroy that value. |
| Helper hygiene | Names beginning `__bream_`, builtin names and raw binding/function identifiers are rejected. Generated runtime paths and primitive types are fully qualified. |

Every loop head suspends once, even if the body is empty. `continue` reaches the next head and cannot bypass that checkpoint. `while` is reconstructed as a checkpointed loop with a primitive condition and a generated break; `for` only uses builtin integer ranges. Conditions and bounds cannot call dependencies, iterator callbacks or arbitrary methods. There is no recursion path because authored function calls are forbidden.

The scheduling policy is **one forced `Pending` per loop iteration**, not a time quantum, reduction-count model, instruction counter or latency promise. A finite straight-line sequence runs until the next generated checkpoint or function completion. This is a native state machine with generated cooperative suspension, not the earlier signal-based prototype.

Arbitrary calls, method calls, macros, external/global reads, unsafe blocks, custom types, closures, function/items/impl declarations, custom `Drop`, and explicit authored `await` are outside the accepted grammar. An attribute that merely appended a checkpoint to otherwise unrestricted Rust would not establish this boundary.

## Negative compilation evidence

The runner invokes Cargo separately for each unsupported example and requires a nonzero exit with a diagnostic from the subset checker. It preserves the full diagnostics. The final fixtures reject:

- A qualified external `std::thread::scope` call.
- A custom type annotation.
- A local type and user `Drop` implementation.
- A read from an external global.
- An arbitrary macro invocation.
- An unsafe block.
- A closure whose body loops forever.
- Explicit manual `.await`.
- A reserved generated helper name.
- A raw identifier attempting to evade the reserved-prefix rule.

Examples are in [runtime/examples](runtime/examples). The raw-identifier rejection was added after independent read-only review identified that raw spellings could evade a plain string prefix/no-shadowing policy. The reviewer found no remaining accepted-body path to arbitrary callbacks, custom destructors, foreign calls or globals in this bounded inspection. This is useful review evidence, not a formal proof of the checker.

## Cleanup and boundary limits

Payload counters measure **logical owned byte reclamation**, not RSS release. Accounting in `OwnedBuffer::drop` precedes normal field destruction; the assertions run after dropping the future has returned, so ordinary `Vec` destruction has completed. The allocator can retain freed pages for reuse.

The runtime accepts at most 4 KiB per buffer and 64 KiB of live tracked buffer payload per process. Buffer allocation still uses the host's standard allocator. It can block internally; allocation failure can terminate the host. An oversized accepted request hits a trusted builtin assertion, and this private executor does not yet contain that panic. Thus this experiment does not establish bounded wall-clock cancellation, allocation/panic isolation or hard cleanup latency. No checkpoint is inserted inside trusted runtime builtins or `Vec` deallocation.

Primitive arithmetic and the opaque buffer are deliberately chosen so the authored program cannot supply an arbitrary destructor or dependency loop. This is the reason normal future destruction is tractable in the demonstrated subset. General Rust `Drop`, guards, borrowed host resources, foreign code and collections with user-defined elements require separate policy; they are not silently admitted.

The concrete generated future representations measured 72 bytes for the infinite process and 64 bytes for the survivor before boxing in this compiler/build. These are **future-object sizes**, excluding the 4 KiB payload, reference-counted metrics/context allocations, executor slots, allocator overhead and any mailbox. They are not complete process sizes or a density benchmark.

The private executable's executor accepts ordinary futures internally and is explicitly not a public “process-safe” API. Only its macro-generated fixtures are qualified. Runtime helper visibility exists for expansion across crates; it is not a sealed safety gate. A product API would need an enforceable admission boundary so an arbitrary future cannot acquire the same guarantees through a safe public constructor. This prototype intentionally avoids claiming such an API exists.

There is no mailbox/message ownership protocol, supervision tree, process linking, general process heap, arbitrary dependency support, multi-worker scheduler, networking or production threat model here. The next question is whether this restricted process language is an acceptable product contract, and which bounded, audited capabilities it should gain.

## Reproduce and audit

From this directory on the current machine:

```sh
python3 run.py --label YOUR_FRESH_LABEL
```

The runner uses the existing lab Rust 1.98.1 toolchain read-only, a local ignored Cargo cache/target directory, a 120-second external build deadline, a 10-second execution deadline, and 60 seconds per negative compilation. Each subprocess starts in its own process group; timeout kills that group, and core dumps are disabled. It records source, manifests, lockfile and executable SHA-256 hashes plus all commands/stdout/stderr. The final stored hashes were rechecked after qualification and all match. Final Rust source passes rustfmt checking.

Ordinary Cargo commands also work with Rust installed:

```sh
cargo build --release --locked --bin bream-probe-runtime
cargo run --release --locked --bin bream-probe-runtime
# Expected to fail with the subset diagnostic:
cargo check --release --locked -p bream-probe-runtime --example reject_custom_call
```

Do not use `cargo --all-targets` as a passing check: the examples intentionally include rejected programs. Final evidence is `bream-final`; the [historical archive](../../evidence/archive/pre-bream/README.md) retains the original source/executables and qualifications from before the rename, including earlier development stages. No original `rust-actor-runtime-lab` files were modified.
