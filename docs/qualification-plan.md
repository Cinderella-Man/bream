# Qualification gates for a native preemptive library

The accepted first scope is [BEAM/ERTS primitives](adr/0004-beam-primitives-first.md); the concrete next experiments are the [P0–P5 primitive roadmap](roadmap.md). These broader qualification gates make the intended product falsifiable. They are a proposed next program, not passed tests unless explicitly marked. No gate can be satisfied by increasing the number of cooperative workers until two tasks happen to run on different threads.

## Gate 0: native automatic switching

**Fresh mechanism evidence exists** in [the native probe](../experiments/native-preemption/README.md). It uses a pinned third-party scheduler rather than a Bream implementation. Two finite native Rust loops must overlap on one task worker, complete with correct results, and differ from the otherwise equivalent preemption-disabled control. A monitor thread is permissible; a second computation worker would invalidate the key comparison.

Add a never-returning compute loop next, with progress observed externally and an OS-process watchdog terminating only the disposable test executable. The watchdog is test containment, not the runtime's scheduling mechanism. Include a loop in a separately compiled dependency and optimized floating-point/SIMD code. Record exact compiler, flags, artifact hashes, task thread identity, final registers/results and preemption events.

## Gate 1: native compatibility and safety argument

**Required addition after the interview:** safe forced termination with runtime-managed cleanup is a first-product requirement. Before selecting an engine, prove termination of non-yielding process code, safe removal of its continuation, complete cleanup of tracked memory/handles/children, and no access to reclaimed state. The current arithmetic probe does not execute a kill operation. An indefinitely parked stack or a stop flag that waits for the application's next voluntary call is not a pass.

**Partial generated-model evidence now exists:** the [second probe](../experiments/generated-process/README.md) stops an infinite admitted process after an automatic checkpoint, reclaims its tracked 4 KiB buffer, and lets its peer finish. An empty infinite loop also suspends automatically; ten unsupported-code fixtures fail compilation. This is evidence for the deliberately closed grammar and buffer lifecycle, not completion of Gate 1.

For a Cargo-generated process model, extend compile-fail and transitive-coverage probes for unsupported native calls, hidden blocking/loops in callbacks and overloaded operators, shared mutable globals, untracked allocations/handles, scoped child threads, escaping borrows and unbounded destructors. No simple source-token rewrite establishes those properties. Compare controlled-state cleanup against ordinary Future cancellation; establish exactly which user types can be safely reclaimed and whether their `Drop` code runs.

| Case | Required observation | What failure means |
|---|---|---|
| Allocating/freeing under interruption | Correct results, eventual completion, no allocator corruption/deadlock | Defer/replace/audit allocator boundary before claiming general Rust support. |
| Runtime mutex with contending processes | Holder resumes; waiter parks without pinning all workers | Fix scheduling-aware synchronization. |
| Ordinary std mutex | Reproduce and document supported behavior and excluded blocking cases | Do not claim transparent std compatibility. |
| TLS initialization, access, borrows, destructors | Declared per-thread/per-process semantics preserved | Pinning alone is insufficient. |
| Rust panic with live destructors | Supported unwind boundary works; other computations progress | Narrow panic configuration or repair unwind handling. |
| Signals and alternate stacks | Nested delivery cannot overwrite suspended context; unrelated handlers coexist | Not ready for application embedding. |
| Blocking calls and EINTR | Explicitly supported calls preserve liveness and error semantics | Hook/offload/handoff policy incomplete. |
| Runtime drop with active work | No stack freed while resumable; documented shutdown completes or reports outstanding work | Lifecycle API unsound or underspecified. |

Keep failing runs and source pins. Sanitizers and Miri can help on supported pieces, but neither alone validates asynchronous signal/context-switch assembly. Write the actual invariant argument and seek independent unsafe-code review before presenting a safe general-purpose API.

## Gate 2: density and fairness

Use both empty waiting processes and realistic stateful workloads. Measure shared baseline, RSS/PSS slope, virtual address space, stack pages touched, process metadata, channel storage, native thread count and teardown retention. Native idle processes, active stackful computations, Tokio tasks and BEAM processes must be compared with equivalent useful state and clearly named differences.

Run runnable populations above worker count, mixed short/long work, spawn storms, parked/waking races and CPU affinity restrictions. Report throughput and p50/p99/max delays as distributions. A successful two-process alternation is not evidence of good latency at 10,000 runnable processes.

## Gate 3: independent library usefulness

Build two small clients against the same kernel: a CPU computation service and a long-lived stateful worker using native typed channels. Neither client imports an actor framework, Wasmtime, Elixir transport, serialization protocol, distributed membership, upgrade machinery, or the original lab's domain types. Host one client alongside Tokio and demonstrate that signal ownership, worker threads, timer threads and shutdown compose.

## Gate 4: adopt, contribute, or implement

Run the same contract against candidate upstream native engines. If an existing engine satisfies the contract, contribute missing packaging/invariants upstream or build a thin, clearly differentiated layer. Fork only when a demonstrated architecture mismatch warrants taking ownership. Implement a new engine only after identifying which tested requirement existing candidates cannot meet.

Failure should change an explicit compatibility or implementation decision. It must not change the accepted no-manual-yields requirement by stealth.
