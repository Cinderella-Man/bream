# Checkpoint placement, continuation state and measurement costs

Source review, 2026-10-07. Reference: OTP `29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. This follows the execution trace in [report 11](11-beam-scheduling-source.md), concentrating on fast/slow branches that could inform a native Rust implementation. **No new VM or Bream execution, compiler experiment or benchmark was performed.** The generated-process approach remains an accepted research route, not a selected production engine. The proposals below require no handwritten yields.

## A checkpoint is not necessarily a context switch

At function ingress the x86 JIT emits a reduction decrement and conditional branch. Only the exhausted branch enters a shared fragment that records the function/arity and dispatches to context switching. Allocation-origin instrumentation conditionally adds an instruction-pointer store. The ordinary path therefore need not run the full scheduler or read a wall clock at each check. These are generated instructions, not an application convention. [Ingress][ingress], [exhausted fragment][yield-fragment].

Return checks also remain in module code. The source comment says moving that reduction test into a shared fragment caused a Dialyzer performance regression; it supplies no reproducible timing in this file. This is a useful warning against assuming that sharing more code is automatically faster. The return branch uses a different condition from ingress (`jl` versus `jle`); this review does not turn either instruction into a universal quantum specification. [Return check][return].

**Bream experiment:** compare an inline counter/branch with an out-of-line check, keeping identical mandatory boundaries and admitted work. Vary straight-line block size, helper depth and the number of distinct generated process bodies. Measure emitted code size, branches, instruction-cache behavior where available, throughput and peer/kill latency. Code sharing may improve density while introducing dispatch cost. Moving or merging boundaries requires a new bounded-work argument, not just a faster average.

## Runtime entry updates selected state

`emit_enter_runtime<Spec>` has mask-dependent heap, stack and reduction updates, plus mandatory stack-switch state. Some combined heap/stack stores use AVX or SSE depending on CPU support. Other paths save or align the runtime stack; frame-pointer mode writes the stack pointer even without the stack-update bit and restores it unconditionally on return. Omitted heap/stack updates in the applicable paths can be poisoned in debug builds to detect a helper using stale state. The leave path combines mask-dependent reloads with those mandatory stack-switch operations and an optional active-code-index update. [Entry masks and debug poisoning][entry], [frame/stack/reduction branches][entry-rest], [return masks][leave].

This is a **representation-specific proof obligation**: ERTS knows which helper may allocate, inspect the stack or consume reductions. It does not establish that arbitrary Rust callbacks can be marked cheap or allocation-free. Bream could generate an internal, closed helper classification and keep common state in locals until a real handoff, but external safe code must not be able to forge that classification. Rust references, partial initialization, pending resource ownership and panic paths must remain valid at every permitted boundary.

A useful comparison has three treatments: a conservative full-state transition, a generated minimal-state transition, and a debug validation mode that detects use of omitted state through safe validity metadata or representation-valid sentinels. Do not manufacture invalid Rust references or initialized values to imitate C debug poisoning. Measure the actual rustc output: an attribute or source-level local variable is not evidence that a register spill disappeared. Do not copy BEAM's architecture-specific assembly into an ordinary-Cargo design by assumption.

## Light helpers still have slow branches

The light-BIF path first checks tracing and saved-call recording, then accounts a reduction, enters runtime state and calls the helper. Its return may trigger garbage-collection work based on flags, binary overhead or heap-fragment pressure; a normal result, continuation trap and error take different routes. The path also refreshes the active code index for BEAM's observable upgrade ordering. [Entry][light-entry], [call and instrumentation][light-call], [return/GC/trap][light-return].

Heavy BIF and NIF shared stubs explicitly rely on checks in their module entry paths; adding another shared call layer does not establish scheduling coverage. Native function execution still has the limitations recorded in [report 11](11-beam-scheduling-source.md). [Heavy-BIF placement][heavy], [NIF placement][nif].

**Bream opportunity:** specialize admitted scalar operations and small protocol matching, and keep uncommon growth/error/control transitions separate. However, a fast path must fall back before exceeding its bounded-work contract; allocation failure, trace mode and control work must remain accounted for. Bream does not need BEAM hot-code-index machinery while code replacement is unsupported. That is scope-specific elimination of unused machinery, not permission to omit supported lifecycle/control synchronization.

## Saved live state affects both speed and process size

On a normal x86 context-switch path, ERTS records the continuation, computes used reductions and calls `copy_out_registers`. An exiting process instead selects its exit continuation. `copy_out_registers` grows saved argument storage only when `arity` exceeds capacity, then copies the live argument count; `copy_in_registers` copies that count back. The first larger save may allocate or reallocate, while repeated saves within capacity avoid that growth. [Switch branches][switch], [save/restore][save].

The relevant Bream question is how much state must survive a boundary, not whether a future or task header is small. Compare a fixed maximum frame with generated per-state layouts and reusable spill storage. Include the first transition to a large live set, subsequent transitions, partial-resource states and process death at each state. Dead scalar values may be omitted; owned resources cannot disappear merely because their application value is never read again. Measure code size as well as memory: many specialized states could trade smaller frames for larger instruction footprints.

## Measurement must separate the work from its observer

BEAM's profiling guide explicitly distinguishes instruments with different overheads; its benchmark guide separates CPU from wall time and warns about heap history and architecture. Those are source guidance, not new measurements here. [Profiling guidance][profiling], [measurement guidance][benchmark]. Bream should retain uninstrumented, counters-only and sampled/full-trace modes with separately named results. A fully traced test can establish ownership-event order while perturbing the timing it is supposed to measure.

For service latency, keep distributions across runs rather than adopting only a best-case time. Schedule offered requests independently of replies as well as running a closed request/reply workload; record intended issue time, actual enqueue time and completion time so a stalled load generator cannot hide stalls. Stop at a finite, declared test horizon under overload and record remaining mailbox/cleanup backlog. This is proposed measurement policy; it does not impose a bounded mailbox on the product.

## Additional build cases

These refine conditional implementation/measurement branches. No exact register layout, quantum constant, hard deadline or checkpoint algorithm is adopted.

| Case | Branch and required treatment | Evidence to collect |
|---|---|---|
| LANG-03.C09 | Inline checkpoint continues versus exhausted branch materializes a continuation. | Same admitted computation and control obligations in both paths; test kill at the branch and forbid partially saved or post-death resumed state. |
| LANG-03.C10 | Helper transition omits selected saved fields versus full-state fallback. | Closed helper classification; debug poison/negative controls detect forbidden reads; allocation, error and panic paths cannot use stale state. |
| LANG-03.C11 | First suspension expands the saved live set; later suspensions reuse capacity or shrink logical liveness. | Track initialized values and resources independently of liveness; no lost ownership, stale reads or invalid partial cleanup after kill. |
| EXEC-02.C10 | Specialized cheap helper crosses to variable-cost growth, tracing, failure or control work. | Re-enter accounted/resumable work before an unbounded operation; qualification covers both sides of each specialization threshold. |
| EXEC-02.C11 | Cached allowance is spilled/reloaded across helper return, suspension and error. | No reset, wraparound, duplicate debit or lost debit grants unbounded execution; changing checkpoint frequency requires a new maximum-segment argument. |
| MEASURE-01.C06 | Cold first-use allocation versus warmed state, and disabled versus enabled instrumentation. | Separate cohorts, retain compiler/configuration/artifact identity and report observer overhead; do not merge modes into one claimed latency. |
| MEASURE-01.C07 | Offered rate exceeds service capacity, or the load generator itself stalls. | Record offered/completed rates, enqueue delay, response time, horizon and retained backlog; preserve growing-mailbox semantics and expose coordinated omission. |

## Concrete optimization experiments

| Local task | Baseline and treatment | Useful result / rejection |
|---|---|---|
| CHECK-P01 | Full helper call per checkpoint versus inline counter and cold scheduler branch at identical boundaries. | Compare arithmetic, branches, helper calls and many code bodies; reject faster throughput with broken control coverage or unaccounted segment growth. |
| CHECK-P02 | Conservative full saved state versus generated live-state layouts and selective helper updates. | Save/restore bytes, first-growth latency, process footprint and code size; reject stale state or missing cleanup ownership. |
| CHECK-P03 | Fixed small work chunks versus cost-aware chunks capped by remaining allowance. | Byte-copy, matching and fanout throughput plus peer/kill tail; reject chunks whose true work grows unbounded with input. |
| CHECK-P04 | Generic admitted helper dispatch versus specialized scalar/protocol paths with explicit slow fallback. | Dispatch cost and code-size frontier; inject growth/error/control at every threshold. |
| CHECK-P05 | Always-on event recording versus disabled, counters-only and sampled observation. | Attribute instrumentation overhead and record loss explicitly; no claim that sampled traces prove every ordering invariant. |

These feed the [performance experiment backlog](../performance-plan.md), where dependencies, shared workloads and acceptance gates are defined. No candidate is selected merely because ERTS uses it. Full compiler coverage, Rust helper admission, safe termination and actual performance remain open.

[ingress]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/instr_common.cpp#L3217-L3241
[yield-fragment]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/instr_common.cpp#L3205-L3215
[return]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/instr_call.cpp#L47-L77
[entry]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/beam_asm.hpp#L699-L759
[entry-rest]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/beam_asm.hpp#L760-L815
[leave]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/beam_asm.hpp#L818-L868
[light-entry]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/instr_bif.cpp#L420-L449
[light-call]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/instr_bif.cpp#L472-L535
[light-return]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/instr_bif.cpp#L537-L598
[heavy]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/instr_bif.cpp#L810-L830
[nif]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/instr_bif.cpp#L971-L1012
[switch]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/process_main.cpp#L161-L203
[save]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_common.c#L306-L343
[profiling]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/efficiency_guide/profiling.md#L24-L42
[benchmark]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/efficiency_guide/benchmarking.md#L83-L113
