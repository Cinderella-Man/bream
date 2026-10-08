# Structured state and automatically scheduled helpers

**Passed for the declared restricted profile, 2026-10-08.** A separate consumer now runs an owned service record, dispatches a tagged request, and moves its state through data-dependent helpers. Authored functions contain no yields or awaits. Generated checkpoints share one process allowance through nested calls. External kill also releases partially constructed state exactly once in the tested cases.

This is the second finite implementation milestone after the [useful-buffer experiment](../process-milestone/README.md). It extends the checked native execution boundary; complete G1/G2 qualification and a production API remain open. [Progress tracker](../../docs/milestones.md), [retained evidence](../../evidence/structured-process/README.md).

## Runnable implementation

The [consumer](consumer/src/main.rs) defines `#[process_module] mod service` with `State { words: Words, digest: u64, processed: u64 }` and `Request::{Mix(u64), Inspect}`. Its `apply` helper owns and returns the record, matches the request, and calls nested `mix` helpers for each word. Independent host code computes the expected result.

The attribute validates and reconstructs the whole private module. It emits native async helper functions and a `program()` factory returning the runtime's opaque `Program`. The [compiler](macros/src/structured.rs), [runtime](runtime/src/lib.rs), and [consumer rejection fixtures](consumer/examples) are separate crates. The legacy buffer compiler and consumer remain regression controls in this workspace; the earlier workspace and evidence are unchanged.

The retained binaries can run individual controls without rebuilding:

```sh
.tools/structured-process/20261008-structured-01/debug/bream-structured-consumer controls 32
```

The control command intentionally triggers two caught process panics and prints their diagnostics. Its JSON outcome must still be `passed`. See the [evidence runner](../../evidence/structured-process/run.py) for pinned, offline, locked rebuilds and fresh uniquely named cohorts.

## Admitted operations, ownership and limits

| Surface | This experimental profile |
|---|---|
| Primitive values | Concrete `u64`/`bool`, wrapping integer arithmetic, comparison and short-circuit boolean operations. |
| Owned buffer | Concrete runtime `Words`; checked `len`/`read`/`write`. Host input capacity is at most 8,192 words, including spare capacity. |
| Private records | 1–8 named fields per record; concrete primitives, words, private records or tags. At most eight data types, nesting depth four and 32 flattened leaves. Recursive records are rejected. |
| Private enums | 1–8 variants, each unit or one `u64`/`bool` payload; exhaustive matching checked by rustc. Owning enum payloads are outside this slice. |
| Private helpers | Owning parameters/results, at most eight arguments and 16 functions; acyclic call graph with maximum static depth eight. All helper bodies are checked, including unused ones. Value helpers use explicit returns. |
| Control flow | Built-in integer ranges, loops, conditionals, checked matches without guards, return, break and continue; native Rust move checking and destruction. |
| Observation | Concrete runtime result/progress/stage operations. Stage marks support deterministic ownership checkpoints in the controls. |
| Rejected execution | External functions/types, imports/aliases, traits/impls/derives, custom `Drop`, closures, macros, statics, references in helper signatures, generics, manual async/await and unsafe authored blocks. |

Each statement and loop head is charged, as is helper entry. All helpers reborrow the caller's existing context; entry does not reset the remaining allowance. The six-step nested-helper control takes six polls at allowance 1 and one poll at 32 or 256. The generated helper calls introduce no per-helper box; their futures live in the containing native frame. The runtime still boxes the outer frame, owns a host-allocated vector and performs other runtime allocations.

Explicit external crate bindings protect emitted primitive/runtime paths against consumer lexical shadowing. Generated data and helpers stay private. The existing documented unsafe factory remains a trusted escape hatch; arbitrary native futures and closures cannot enter through ordinary safe spawning.

## Cancellation and partial construction

All four cases run in both compiler profiles at allowances 1, 32 and 256. Each reaches its named stage, progresses through multiple polls while a peer runs, then receives external kill. Each releases its 1,024-byte input once and remains stopped after another peer finishes.

| Suspended case | Input ownership at kill | Observed frame bytes |
|---|---|---:|
| First record field initialized, later field's helper active | Partial record temporary | 232 |
| First authored initializer's helper active, owning initializer still later | Process context | 200 |
| Complete record moved into infinite helper | Helper frame | 336 |
| First owning argument evaluated, later argument's helper active | Pending call argument | 232 |

Additional controls cover panic during partial construction, panic inside a helper owning a record, nested record fields, both enum branches, short-circuit evaluation, ordered argument evaluation, empty and nested infinite helper loops, shutdown, and the earlier buffer controls. The two panic paths also release input exactly once and leave the worker able to run a peer.

Review found a generated-borrow conflict in `result(helper())`: borrowing the observation receiver overlapped the helper's mutable context borrow. The compiler now evaluates the argument into a scoped temporary first. A distinguishing `result(combine(first(), second()))` control checks both the borrow fix and left-to-right helper argument effects.

These controls cover 55 rejected consumers in both debug and release (33 new, 22 inherited), four positive shadowing executions, six structured control invocations and six baseline invocations. They are concrete admission and ownership observations, not exhaustive language or concurrency proofs.

## Structural opportunities and performance reset

**Performance evidence removed, 2026-10-08, at the user’s request. All speed tests must be redone in an authorized suitable environment. No speed tests may run on this machine.**

Generated helper futures live in the containing native frame and add no per-helper box. Owning state still affects frame layout; the retained correctness controls above identify the live owners. Initializer order is observable and cannot be changed as a purported optimization without preserving behavior. These are structural observations, not speed claims.

## Limits and next boundary

The profile has one application worker and a host control thread. Stop is published before killed-frame cleanup; natural completion or panic may destroy values within a poll. Host allocator and panic-hook behavior remain assumptions. No wall-clock cleanup or responsiveness bound is established.

No new BEAM execution, message transport, reusable identity registry, waiting I/O or full runtime qualification is claimed. Responsiveness remains the user's first priority. The next bounded milestone is stable process identity and terminal lifecycle, including stale handles, slot reuse and publication/cleanup ordering, before introducing mailbox ownership.
