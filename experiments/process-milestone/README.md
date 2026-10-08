# Useful native process milestone

**Passed for the restricted profile below, 2026-10-08.** A separate Cargo consumer now submits checked native programs through an opaque public `Program`, transforms real owned input, shares one worker with an infinite computation, requests kill from the host, and observes stopped execution and completed cleanup separately. This is an executable slice of G1/G2, not the complete process library. [Milestone criteria](STATUS.md), [project progress](../../docs/milestones.md).

The useful [consumer](consumer/src/main.rs) contains an infinite 1,024-word transform and a finite 8,192-word transform/checksum. Both mutate owned buffers. The peer's result is checked independently, the hog makes repeated progress while that peer runs, and a further peer runs after the hog is killed. The killed handle's final observations must remain unchanged. Process bodies contain no authored async, yield, cancellation check or runtime checkpoint.

## What changed from the first mechanism

The old probe accepted ordinary futures into a private executor and kept its buffer opaque. This separate workspace adds a [public construction boundary](runtime/src/lib.rs), useful indexed data operations and renewable checked-work allowances. It retains native Rust state machines rather than an instruction interpreter.

| Supported operation | Execution and ownership contract |
|---|---|
| `owned_input()` | Moves the one concrete `Vec<u64>` from the runtime context into the process. Host input and frame allocation precede submission timing. A second take fails the process through the tested panic path. |
| `len(words)`, `read(words, index)`, `write(words, index, value)` | Lowered to trusted inherent methods on the real runtime type. Indices/values are checked primitive expressions; no user indexing, conversion or iterator callback. Out-of-bounds access fails the process in the tested host configuration. |
| Primitive locals, wrapping arithmetic and checked control flow | The earlier closed grammar is retained. Every statement and loop head has a generated budget check; `continue` and empty loops cannot evade suspension. A step is an experimental accounting unit, not a CPU instruction or time limit. |
| `observe(value)`, `result(value)` | Runtime-owned scalar observations; their instrumentation cost is included. |
| Input and cleanup | Capacity is limited to 8,192 words; tracked bytes are capacity × 8, not initialized length or RSS. Input cannot be moved into unknown code or given custom element destruction. |

Only `Program` enters safe spawn. Its fields are private; the sole factory escape hatch requires an explicit unsafe call with documented execution/cleanup obligations. The macro validates and reconstructs the whole body, then invokes that constructor using a noncapturing factory. Generated modules explicitly bind the actual runtime, core and std crates. The positive shadowing fixture aliases the caller as all three and must still use genuine types and checkpoints. Actual dependency replacement and host allocator/panic-hook configuration remain part of the trusted build/host environment.

The executor has one application worker and a separate host control thread. It resets the allowance per poll and runs ready programs round-robin. All admitted `Pending` states in this slice are budget exhaustion; there are no I/O or receive waiters. Kill sets a control flag independently of application consumption. Only the worker commits stop, after any active poll returns; it drops the frame and publishes cleanup afterward. Runtime shutdown rejoins that worker after draining tasks. This is checkpoint-based stop, not arbitrary-instruction destruction.

## Correctness evidence

The [filtered corrected cohort](../../evidence/process-milestone/runs/20261008-useful-process-02/results.json) retains 57 correctness/build commands: six consumer controls across both profiles, two shadowing executions and 22 expected rejected consumers per profile, plus builds. These include queued kill, loops, bounds failure with surviving peer, shutdown and input capacity/accounting. The initial cohort remains as a correctness record of the admission gap later fixed.

**Performance evidence removed, 2026-10-08, at the user’s request. All speed tests must be redone in an authorized suitable environment. No speed tests may run on this machine.**

All release measurements and derived allowance/throughput/latency claims have been removed. Useful-work measurement scenarios remain in source for future redesign, but their deleted observations cannot qualify correctness or performance. [Filtered evidence](../../evidence/process-milestone/README.md).

## Remaining boundaries

User-defined structs/enums, reusable helper calls, messages, receive/timers, waiting-state kill, reusable process identities and multiple workers are still absent. Unknown methods/macros/globals/custom destruction remain rejected. A finite expression can contain substantial work; host allocation/deallocation and custom host hooks are not hard-time-bounded. The index-failure control covers the tested default hook/allocator, not arbitrary host callbacks.

Zero destruction-during-poll applies to these killed fixtures, which retain their input. An admitted block may legitimately release its input when that block finishes during polling. Natural completion or contained failure can also destroy owned locals during the poll. The invariant is that the runtime never destroys an actively executing frame.

The next milestone is useful checked structured state and helpers through this same public boundary, including partial-initialization and cleanup controls. Further BEAM source reading is tied to that gate. Full G1/G2 remain open; no mailbox/lifecycle feature is reported as complete.
