# Selective receive markers and priority cursor protocol

**Execution follow-up:** [Report 29](29-pinned-reference-observations.md) now records targeted observations on the exact pinned build. The source-only status and hypotheses below describe the original review stage; the follow-up identifies which variants were executed and which remain unresolved.

Source review, 2026-10-07. Pinned OTP `29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. This extends [report19](19-process-signals-timers-deep-dive.md) with the product of compiler freshness proofs, runtime marker lifetime and priority cursor transitions. **No VM, compiler, test suite or fixture was executed.** Branch cases below are planned; source predictions and a possible assertion inconsistency remain unconfirmed.

## The marker is evidence about a position, not a message

`ErtsRecvMarker` shares signal linkage but has four distinct types: receive, yield, priority-end and priority-continuation. Only receive markers belong to the process's eight-slot `ErtsRecvMarkerBlock`, whose parallel `ref[]` array associates slots with temporary IDs or references. Priority markers belong to a separate `ErtsPrioQInfo`, with a capability count and saved cursor mode. The same representation does not imply interchangeable ownership or destruction. [Representation][representation], [priority storage][pq-struct]; local entries: [marker types](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_message.h:296), [cursor protocol](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.h:1681).

Crucially, a receive marker's `in_prioq` means **its reference has been seen in a priority message**. The receive marker itself stays outside the priority prefix. The bit is conservative historical evidence, not a live reference count or physical-location flag. Insertion searches references not previously seen; it resets the bits when beginning a new empty-prefix epoch. Destruction of priority metadata clears them too. Clearing one matching message need not clear its bit. [Reference tracking][track-ref], [metadata destruction][destroy-pq].

The eight `ref[]` entries are GC roots, and heap relocation adjusts them. A bounded native metadata block can retain VM terms; “not a user message” does not mean “no managed lifetime.” Bream may use stable IDs rather than moving terms, but must still state which metadata owns each reference. [Root registration][roots], [relocation][relocation].

## Compiler admission is stronger than mentioning a reference

The compiler reserves a boundary **before** a recognized fresh-reference operation, then binds its result. Otherwise an immediate monitor reply could precede the recorded boundary. Candidates include `make_ref`, `alias`, `monitor`, extracted `spawn_monitor` references and `spawn_request`; this pass deliberately does not assume the future shape of `spawn_opt` results. [Freshness rationale][rationale], [recognized calls][calls].

For an optimized receive, every message-removal path must establish equality between that reference and the message or something derived from it. A guard comparing two unrelated variables does not qualify; neither does one branch of a guard disjunction when another accepts messages without the reference. `pu_is_ref_used_is` accepts removal only after `ref_matched=true`; failure/next-message and timeout paths do not constitute matching. Module-local argument/return data flow propagates candidates, but this is not a proof for arbitrary application-generated correlation IDs. [Clause proof][clause-proof], [call propagation][call-flow], [negative guard fixtures][negative-guards].

Optimization absence must preserve semantics. Unknown/unbound references and inappropriate values make marker use/clear harmless; a full pool may lose an optimization. The compiler intentionally does **not** clear every marker on exceptions: it relies on later recycling. This is distinct from resetting the active receive cursor, which is required when leaving a receive. [Optional protocol][rationale], [exception policy][exceptions]. For Bream, begin with a correct full scan; only generated code with an equivalent proof may skip a prefix.

## Transition and ownership inventory

| Trigger / state | Mutation and invariant | Ownership / evidence |
|---|---|---|
| Reserve | Fetch incoming signals under MSGQ first. If inner and middle are empty, return `undefined`; otherwise append a marker at their current end. | Temporary integer ID belongs to the marker slot; it can require a bignum. [API][api], [allocation][allocate]. |
| Bind | Replace a matching temporary ID with an internal reference. Invalid binding makes the slot unused and ages it for removal; missing ID does nothing. | A slot cannot be interpreted merely by an application integer. [API][api]. |
| Insert with middle signals | Append to middle and non-message linkage; preserve message offsets. With no middle, append to inner and repair an end cursor that would otherwise point at a marker. | Marker storage remains process-owned throughout movement. [Insertion][insert]. |
| Use before middle handling | Set save to inner end, set `set_save`, record one `pending_set_save_ix`. Reusing or resetting save clears the previous pending request. | The unhandled marker controls a future cursor write; it is not already a received item. [Pending use][pending], [cancel pending][clear-pending]. |
| Handle middle marker | Active marker becomes inner; a pending request sets save immediately after it and clears pending state. A marker marked for removal is unlinked/deallocated; an internal save-end marker can set save then disappear. | Ordinary markers here assert no corresponding priority-reference observation yet. [Signal transition][middle]. |
| Clear / age / reclaim | Clear unbinds and sets pass age, without immediate unlink. Passing ages receive markers; leading markers and over-age markers are removed. A middle marker requested for deletion is marked `in_sigq=-1` until safe handling. | Last receive slot deallocation frees the block; priority/yield nodes use different handling. [Clear][api], [pass][pass], [dequeue][dequeue]. |
| Pool pressure | Prefer an eligible older unused marker already in inner; otherwise reuse an older active one. If the oldest is still in middle, allocation can return no marker. | Reuse removes old linkage first. Losing a marker permits slower scanning, never loss of a match. [Reuse][reuse]. |
| Process death | Exit handling removes priority metadata/markers; middle receive markers are deallocated as exit signals are drained. Final queue cleanup separately recognizes remaining receive markers. | No marker becomes application data; priority storage must not be double-freed through generic cleanup. [Exit][exit], [middle exit][middle-exit], [final queues][cleanup]. |

Pointer repair is part of the protocol. Inserting/removing a priority continuation while handling signals may touch the processed, soon-to-be-inner part of middle. Helpers repair `next_nm_sig`, middle tail, inner tail, save and a following marker's back-link as applicable. A naive list splice that updates only `next` would leave a stale continuation or signal traversal pointer. [Insert repair][pq-insert], [remove repair][pq-remove].

## The four cursor modes constrain priority insertion

`FIRST` records a scan beginning at mailbox start; `RCVM` identifies a receive slot; `LAST` belongs to a known ERTS internal reply wait; `MARK` temporarily diverts a scan through newly arrived priority messages while retaining its prior mode. These describe the scan's origin, not just the address of its current pointer. [Mode definitions][modes].

| Priority accepted while… | Required cursor action |
|---|---|
| Save already inside priority prefix | Append before the priority-end marker. Save is already before the insertion point; preserve its continuation. |
| Save outside, mode `FIRST` | Insert/move continuation at the current ordinary position, record `FIRST`, switch to `MARK`, and redirect to the old priority end immediately before the new message. This does not rescan the entire old priority prefix. |
| Save outside, mode `RCVM` | Redirect only if the tracked reference may be in the priority prefix. Newly inserted terms can change that evidence; unrelated priority traffic need not disturb the optimized wait. |
| Save outside, mode `LAST` | Do not redirect. The implementation relies on these private-reference replies never being priority messages. This is not a general rule for user request/reply. |
| Save outside, mode `MARK` | Invalid state in the implementation: a continuation diversion implies save is still inside the priority prefix. |

Source: [insertion decision and cursor update][redirect]. Reaching the end in `MARK` restores the stored mode and removes the continuation. Reaching it in `RCVM` returns after the designated marker, or reinstates deferred positioning if that marker still needs middle handling. Reaching it in `LAST` is an internal error. [Continuation restoration][restore].

Priority reference detection walks lists, tuples, map keys/values and closure environments, comparing full internal reference identities. It is intentionally conservative: occurrence does not prove a receive clause will match. The traversal's input-sized cost remains unbudgeted inside this function. Rust typed envelopes need either complete traversal of all admitted reference-bearing structures or conservative rescanning; looking only at a top-level request field would need a stricter protocol admission proof. [Walker][walker], [reference equality][ref-equality].

## No match, receive completion and exceptions differ

One rejected message advances save and passes metadata; it does **not** reset to the beginning. Matching removal resets save, as do timeout continuation and invalid-timeout errors. Reset first cancels pending middle-marker positioning, then handles leading metadata and sets `FIRST`. Thus a later receive must revisit retained ordinary/priority messages even if the earlier receive exhausted its scan. The x86 JIT uses the same marker helpers; shared JIT timeout/removal paths perform the reset. This review sampled x86, not all backends. [Advance/reset][advance-reset], [x86 instructions][jit], [match reset][match-reset], [timeout reset][timeout-reset].

A caught exception before entering the receive may leave an orphan marker, by compiler design. An exception from a selected body occurs after removal/reset. These should not be collapsed into “every exception clears all markers.” Upstream exception/recursion tests intentionally disturb an outer marker and then require the original reply still to be received; invalid-timeout regressions require a subsequent receive to recover retained messages. [Exception fixtures][exception-tests], [timeout regressions][timeout-tests].

## Capability removal exposes a concrete qualification gap

Removing the final priority capability clears `FS_PRIO_MQ`, but retains metadata while the end marker remains. Re-enabling reuses that metadata. The end marker is removed only when it is leading during a `FIRST` reset/pass; merely scanning past unmatched priority messages does not discard them. [Capability lifetime][capability], [end-marker removal][pass].

**Unexecuted assertion hypothesis:** optimized marker use with `in_prioq=true` and an existing priority prefix asserts `FS_PRIO_MQ` in the header. That appears stronger than the legal retained-prefix state after the final capability is removed. A reference fixture should distinguish a debug-only assertion failure, a release semantic issue and an unreachable proposed state; none is established yet. [Use assertion][pending], [capability removal][capability].

Proposed deterministic setup: enqueue an ordinary sentinel; create `R=make_ref()`; create the sole priority alias; send `{R,reply}` through it with priority followed by an ordinary barrier from the same sender; receive only the barrier, leaving sentinel and reply queued; deactivate the alias; then receive `{R,reply}` and finally the sentinel. The barrier proves signal handling has passed the reply. Disassemble first to verify that `R` still has an active generated reserve/bind/use path, then compare normal and debug VM builds and a `no_recv_opt` compilation. Do not report this as a BEAM bug from source alone.

The separately authorized [priority marker probe](/home/bat/projects/bream/evidence/process-protocol-review/priority_marker_probe.erl) implements this setup for the parent investigation to compile/run. This source-review task only wrote it; it does not supply an execution result.

## Ten additional planned cases and executable oracles

Dependencies: `REF-02`, `MSG-02`, `MSG-04`, `MSG-06`, `MSG-07`, `LANG-03`; death cases also `LIFE-05`/`EXEC-03`. These extend coverage, without making priority support an implicit first-version requirement.

| Case | Product to exercise | Acceptance oracle / forbidden outcome |
|---|---|---|
| MSG-06.C05 | `RCVM` × relevant/unrelated priority payload; reference nested in tuple, list, map and closure environment. | Optimized/unoptimized selection and remainder agree. Forbid missing a relevant priority reply; measure unnecessary scanning separately. |
| MSG-06.C06 | `FIRST→MARK→FIRST` or `RCVM→MARK→RCVM`; multiple priority arrivals during scanning and waiting. | No duplicate/lost selections or skipped ordinary suffix. Instrument cursor steps to check continuation restoration rather than relying on elapsed time. |
| MSG-06.C07 | Last capability removed with unmatched prefix and active receive reference; re-enable before versus after drain. | Existing priority messages remain ordered and selectable. Run the assertion hypothesis above; record debug/release outcomes without choosing one in advance. |
| MSG-06.C08 | Internal `LAST` wait crossed by priority messages and its ordinary private reply. | Internal operation completes; priority messages remain for the next user receive. Forbid exposing `LAST` as an arbitrary application shortcut. |
| MSG-06.C09 | Reference observed in one priority epoch, its message consumed, prefix later drained/recreated. | Conservative stale evidence may cost scans but cannot lose messages. Check bit reset/new epoch through instrumented state, not public mailbox length alone. |
| MSG-07.C03 | Fresh-reference receive versus guard alternative not requiring it; immediate monitor reply; helper also called with non-reference tag. | Confirm instruction presence/absence and equal behavior. Forbid skipping an old message accepted by any unqualified clause. |
| MSG-07.C04 | Marker in middle; use superseded by another use, normal receive or timeout; later marker handling. | At most one pending cursor writer. Forbid a stale marker overwriting a subsequent receive's save position or appearing as a message. |
| MSG-07.C05 | More than eight nested candidates, explicit clears, caught exceptions, GC/relocation and full middle-marker pool. | Fall back without semantic change; bound metadata, retain live terms safely, and reclaim orphan slots. Performance loss is distinct from correctness failure. |
| MSG-07.C06 | No-match advance followed by match, finite/zero timeout or invalid timeout; then another receive. | Retained messages remain selectable from the proper start; invalid timeout cannot leave save on metadata. Do not assert a universal message/expiry race winner. |
| MSG-07.C07 | Kill with inner/middle markers, pending save, priority end and continuation. | Application execution never resumes; runtime-owned marker storage is eventually reclaimed once. Assert stopped execution separately from memory return. |

Compile equivalent fixtures normally and with `no_recv_opt`, verify their target functions' disassembly, and compare selected-message sequences plus complete remaining mailbox. The compiler explicitly gates this pass, and its own suite checks marker instruction presence/absence. [Pass switch][pass-switch], [compiler oracle][compiler-test]. Internal cursor states additionally need instrumentation or a state model; behavioral equality alone cannot prove which branch ran.

Selected upstream tests cover nested pressure, exceptions, invalid timeouts and priority arrival during a million-message scan. Their context shares many cases between ordinary and priority delivery. However, the deferred-save fixture explicitly notes that its intended receive optimization was disabled; its name or successful run would not prove that branch is exercised today. Priority `qmarkers` tests mainly assert capability state across lifecycle changes, not the complete marker/cursor product. [Shared contexts][contexts], [pressure fixture][pressure], [priority arrival][arrival], [deferred warning][deferred], [capability assertions][qmarkers].

Remaining gaps: execute the differential/debug fixtures; qualify pending-middle states under controlled scheduling; prove complete compiler data-flow correctness and every backend; budget term traversal; exhaust tracing/heap-adjustment yield-marker interactions. This is a narrower transition inventory with falsifiable oracles, not exhaustive verification.

[representation]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.h#L296-L332
[pq-struct]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L241-L258
[track-ref]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L9313-L9355
[destroy-pq]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L9064-L9101
[roots]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L2695-L2699
[relocation]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L3521-L3525
[rationale]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/compiler/src/beam_ssa_recv.erl#L27-L105
[calls]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/compiler/src/beam_ssa_recv.erl#L358-L395
[clause-proof]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/compiler/src/beam_ssa_recv.erl#L538-L663
[call-flow]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/compiler/src/beam_ssa_recv.erl#L280-L312
[negative-guards]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/compiler/test/receive_SUITE_data/ref_opt/no_1.erl#L7-L97
[exceptions]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/compiler/src/beam_ssa_recv.erl#L225-L237
[api]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.h#L2203-L2299
[allocate]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4083-L4144
[insert]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4290-L4347
[pending]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.h#L2123-L2200
[clear-pending]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.h#L1892-L1903
[middle]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6893-L6946
[pass]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4431-L4478
[dequeue]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L3999-L4077
[reuse]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4148-L4239
[exit]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7258-L7272
[middle-exit]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7449-L7454
[cleanup]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L9588-L9623
[pq-insert]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L8970-L9002
[pq-remove]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4390-L4425
[modes]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.h#L1681-L1710
[redirect]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L9358-L9455
[restore]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4483-L4586
[walker]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L9138-L9297
[ref-equality]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L9105-L9135
[advance-reset]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.h#L2330-L2393
[jit]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/instr_msg.cpp#L35-L79
[match-reset]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/beam_jit_common.cpp#L1167-L1171
[timeout-reset]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/beam_jit_common.cpp#L1215-L1255
[exception-tests]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/receive_SUITE.erl#L221-L284
[timeout-tests]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/receive_SUITE.erl#L694-L757
[capability]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L9501-L9562
[pass-switch]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/compiler/src/compile.erl#L1845-L1847
[compiler-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/compiler/test/receive_SUITE.erl#L413-L448
[contexts]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/receive_SUITE.erl#L61-L85
[pressure]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/receive_SUITE.erl#L621-L692
[arrival]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/receive_SUITE.erl#L798-L850
[deferred]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/receive_SUITE.erl#L287-L355
[qmarkers]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/signal_SUITE.erl#L2274-L2408

Plan integration: these additional IDs are recorded in the [build plan](../build-plan.md); conditional scope and unexecuted ownership/interleaving obligations remain explicit.
