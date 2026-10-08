# Process inspection, control metadata and diagnostics

Deeper source review, 2026-10-07. Reference: OTP `29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. This report follows process inspection and control branches in the implementation, plus selected regression tests. No reference tests were run. It extends [the original source map](../beam-source-map.md); it does not certify every diagnostic backend or every process-info item.

These facilities concern the process runtime itself. Their inclusion in research does not require porting Erlang's application libraries, term display formats or debugger UI. The initial Bream slice already needs safe inspection; broader compatibility remains a separately labelled extension.

## Liveness is an ordered operation

`is_process_alive_1` has four materially different branches: self returns true; a missing local PID returns false; an ordinary present process can take a lookup fast path; an exiting process or one with queued signals takes a request/reply path. The latter preserves order when the caller previously sent an exit that has not yet been handled. Before returning a lookup result, the caller can also be made to process incoming signals from the inspected process. An external PID referring to an old incarnation of this node returns false, while an unsupported remote PID produces badarg. [Implementation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_info.c#L4049-L4117).

Therefore a Bream liveness query claiming this contract cannot simply read a concurrent hash map. It must respect preceding signals and the death visibility boundary. A true result is still an observation, not a lease keeping the process alive for a subsequent send. The reference distinguishes an existing process-table entry from an alive process: an exiting process may appear in `processes()` while an alive query resolves false. [Existence distinction](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L5583-L5604).

## Process information has a substantial state machine

The traced path is `process_info_1/2 → process_info_bif → direct inspection or process-info request → erts_process_info/process_info_aux`.

| Branch | Source behavior and consequence |
|---|---|
| Item versus item list | Parse each key, accumulate required locks, result space and handling flags. Invalid keys or malformed list reach badarg. Duplicate requested items and their order remain meaningful. |
| Inspect self | Flush relevant self signals first in this implementation; the flush can consume allowance, yield, or discover a fatal exit. This is explicitly a historical implementation choice, not a separately guaranteed self-signal protocol. |
| Invalid/nonlocal identity | Unsupported nonlocal PIDs fail; an old local-node incarnation can return undefined. Preserve the distinction between invalid argument and absent lifetime. |
| Target immediately inspectable | Try the required locks on a signal-free process. Absence returns undefined. |
| Target busy, exiting, forced signal mode, or pending control | Send a request instead of reading partially changing process state. Message-queue inspection fetches incoming signals and falls back if control work remains. |
| Target exits during direct inspection | Recheck state; either return undefined after final removal or take the ordered signal path rather than returning a stale successful snapshot. |
| Request path | Create a fresh reference, move the receive save pointer, enqueue request, and await response. Failed enqueue resets the cursor before returning undefined. |

These branches are explicit in [argument/target handling](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_info.c#L1274-L1448), [request and cursor rollback](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_info.c#L1464-L1503), and [self-flush continuation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_info.c#L1198-L1271).

Information gathering is runtime work, not an application handler. In particular, a waiting process briefly executing an information request can still report `waiting`; the runtime adjusts that result to the status it would have had without the inspection. [Status branch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_info.c#L1658-L1688), [regression assertion](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L1317-L1337).

Requesting messages can allocate and copy an entire mailbox; requesting the dictionary can copy process-owned data. Work is charged, but those loops do not by themselves demonstrate interruptibility of each large copy. The message-inspection path can also discard malformed distributed messages while preparing the queue. An inspection operation is consequently neither free nor universally observational at the physical storage level. Bream's typed payloads also need a policy for exposing contents without granting mutable aliases or invoking arbitrary `Clone`/formatting callbacks. [Message copy and accounting](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_info.c#L1690-L1754), [dictionary copy](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_info.c#L1963-L1974).

The API has compatibility details worth recording, even if Rust uses a more explicit result type: missing `registered_name` produces `[]` for the singleton query but a `{registered_name, []}` entry in a list query; `process_info/1` has a changeable item set/order; `/2` preserves requested item order and duplicates. Heap words, stack size, reductions, mailbox length and total process memory are different metrics. Parent identity is origin metadata, not a parent-owned lifetime or restart policy. [Public contracts and items](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L8449-L8682).

The upstream mailbox-length/control race test itself warns that it can pass with the bug present and may only trigger a debug assertion. Reading or running that test alone is not proof of the race contract. [Test limitation and fixture](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L734-L795).

## Enumeration has two different consistency contracts

`processes/0` delegates to `erts_ptab_list`. Its implementation explicitly aims at a snapshot of entries that existed at one instant during the operation. It accounts for deletion during incremental enumeration and excludes later creations; the enumeration can trap with a continuation containing a magic reference and partial results. Cleanup for abandoned enumeration must release that bookkeeping. This is not an invitation to hold the entire process table locked for an unbounded list construction. [Snapshot and continuation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L738-L811).

`processes_iterator/0` and `processes_next/1` intentionally have a weaker contract. A process alive for the entire traversal must be included, but the combined results need not equal a snapshot from any single instant. The native step scans a bounded portion tied to remaining reductions, returns a chunk and next index, and rejects an invalid index. Distinguish this guarantee from both snapshot enumeration and arbitrary lossy sampling. [Iterator contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L5606-L5672), [native step](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L1473-L1553).

## Flags, sensitive data and control requests

`process_flag/2` validates and dispatches flags separately: trapping, priority, heap policy, mailbox allocation, sensitivity and distributed signaling are not one generic unchecked option map. Many return the previous value. `process_flag/3` is narrower: the documented remotely settable flag is `save_calls`, implemented using a process RPC for another local process. That request copies its arguments, performs the operation only if the target has not entered exit, and releases its copied off-heap ownership. [Flag dispatch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L2028-L2230), [request ownership](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L2243-L2298), [public subset](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L5506-L5535).

Sensitivity suppresses tracing and selected data inspection; it does not erase process state or create an in-process security boundary. Existing saved calls are not cleared; send/receive/timeout entries can still be added. Whole-dictionary and message inspection return empty results; dictionary-key lookup returns undefined. The separate label branch reads its dedicated dictionary key without the same sensitivity check, so do not infer that all metadata is hidden. Exact label visibility should have its own fixture if Bream exposes that compatibility surface. [Flag contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L8259-L8305), [key/label branches](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_info.c#L2284-L2316), [suppression tests](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/sensitive_SUITE.erl#L392-L424).

## Runtime event observation is distinct from process monitoring

An ordinary process monitor reports lifetime end. System monitoring reports events such as long execution, large heaps and mailbox growth. Legacy `system_monitor` replaces settings and has one receiver; modern `trace:system` is scoped to trace sessions. The native backend shares session handling and validates event settings. Delivery failure to a monitor that disappeared can clear monitoring settings. [Legacy contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L6450-L6500), [session backend](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_trace.c#L3400-L3420), [failed delivery](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L2531-L2567).

Long-mailbox monitoring uses two thresholds: enter at Enable, leave at Disable, with `0 <= Disable < Enable`. It emits state changes rather than a new event for every additional message. This is observation, not mailbox capacity or backpressure. Long-schedule monitoring records long uninterrupted execution; it does not interrupt it or impose a deadline. Profiling runnable transitions also excludes ordinary preemption followed by reinsertion, so that event count cannot serve as a count of all scheduling turns. [Event semantics](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/kernel/src/trace.erl#L1299-L1355), [profile semantics](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L6538-L6560).

Trace-pattern matching, every trace-session lifecycle branch, sequential tracing, crash-dump generation and native tracing backends remain only partially covered. They are process diagnostics, but porting their full Erlang-facing formats is not a prerequisite for native scheduling. Any future Bream trace callback must itself respect bounded runtime work and ownership; an arbitrary callback is not safe merely because it is diagnostic.

## Additional branch cases

All cases are planned. OBS-01 strengthens initial inspection where applicable; broad API compatibility and OBS-02 remain extension research.

| Case | Branch and required qualification |
|---|---|
| OBS-01.C01 | Self/live/dead/old-node/nonlocal alive queries; preserve error versus false and causal order after exit. A true result is no future-liveness guarantee. |
| OBS-01.C02 | Inspect running, waiting, busy, signal-pending or exiting target; qualify direct/request paths and truthful waiting status during inspection. |
| OBS-01.C03 | Self inspection flush yields or discovers exit; finish or stop without lost cursor, invalid GC state or retained request data. |
| OBS-01.C04 | Invalid item/list, duplicate items and missing registration; document exact Rust result mapping and keep list ordering. |
| OBS-01.C05 | Large mailbox/dictionary inspection and target death; account copied bytes and runtime work, reject arbitrary payload callbacks and preserve ownership. |
| OBS-01.C06 | Snapshot enumeration while processes spawn/exit; return a permitted snapshot and safely release abandoned enumeration state. |
| OBS-01.C07 | Weak iterator under churn, invalid iterator and completion; include every lifetime spanning the whole traversal without promising a global snapshot. |
| OBS-01.C08 | Parent/current identity and counters across suspension/slot reuse; expose the correct incarnation and distinguish origin metadata from lifetime ownership. |
| OBS-02.C01 | Toggle sensitivity around tracing/inspection; suppress specified data, preserve state, and separately test labels and earlier saved events. |
| OBS-02.C02 | Set call-history capacity to zero/nonzero/out-of-range, locally or on dying target; preserve prior-value/reset/error behavior and release request copies. |
| OBS-02.C03 | Mailbox crosses enable/disable thresholds repeatedly; emit hysteresis transitions without imposing mailbox capacity. |
| OBS-02.C04 | Diagnostic receiver dies or trace session changes while events are in flight; no dangling receiver, leaked message or stale-session effect. |
| OBS-02.C05 | Long scheduling/GC and repeated runnable transitions; distinguish diagnostic events from enforcement and from actual scheduling-turn counts. |
| OBS-02.C06 | Tracing/inspection creates substantial work or invokes a native callback; qualify overhead and cleanup without compromising admitted-process guarantees. |

The remaining proof obligations are concrete: complete per-item snapshot semantics, scoped trace-session teardown, bounded copying/formatting, and executable reference fixtures. None of the inspected tests supplies a Bream pass result.

Plan integration: these case IDs are recorded in the [expanded build catalogue](../build-plan/process-runtime-expansion.md). All remain planned, with conditional applicability preserved.
