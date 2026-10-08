# Process enumeration and observer lifetime

Research date: 2026-10-07. Reference: Erlang/OTP `OTP-29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. This is a source audit, not an executed test or performance result. It extends [report 21](21-process-inspection-control-deep-dive.md); process-record lookup retention remains in [report 27](27-process-lookup-reclamation-protocol.md) and publication in [report 39](39-process-table-and-spawn-publication.md).

The useful distinction is between **remembering which processes existed**, **keeping observer metadata usable**, and **keeping application execution alive**. Neither an enumeration result nor a trace record is a lease on the application. Full enumeration and BEAM-compatible trace sessions remain later/reference surfaces. Initial Bream observation still needs correct identity, ownership, bounded-work investigation, and the accepted stopped-before-`Down` contract. This report does not select an enumeration API, tracing framework, admission mechanism, or reclamation implementation.

## 1. A consistent snapshot without retaining every process body

The public `processes/0` documentation includes existing *exiting* processes, even though `is_process_alive/1` is already false. The implementation describes its goal as one consistent process-table snapshot from a time during the call. This is neither a guarantee that every result remains addressable at return nor a snapshot of each process's fields. [Enumeration contract][S01] [Implementation entry][S02]

`erts_ptab_list` creates a magic binary containing its continuation state and destructor. It either completes directly or traps with that reference and a partial result. Initialization allocates chunk metadata, takes the table write lock, samples the population count, and allocates an ID array. For a multi-chunk table it appends an invocation marker to the shared deleted-entry list and advances the interval counter. That interval establishes the snapshot cut; it is not literally the earliest instruction of the BIF. A single-chunk table needs no marker because its scan remains in the initial lock interval. [Entry and trapping][S02] [Initialization][S03]

Each subsequent table chunk records its own scan interval under the write lock. The table pass accepts only entries whose creation interval precedes the invocation cut. Concurrent deletion records the physical slot, ID, insertion interval and deletion interval in a separate compact node. It does **not** retain a `Process *`, process heap or stack in that node. The later history pass recovers a pre-cut entry if it died before its slot's chunk was scanned. A deletion after the chunk scan must not append the ID again. [Deletion record][S04] [Table scan][S05] [History predicates][S06]

| Interleaving | How the traced snapshot path handles it |
| --- | --- |
| Pre-cut process remains present through its chunk scan | ID is copied from the table. |
| Pre-cut process dies before its chunk scan | Deleted-entry interval test recovers its ID. |
| Pre-cut process dies after its chunk scan | History sees that the table scan already covered it; no duplicate. |
| A new incarnation occupies the slot after the cut | The table's creation-interval filter excludes it; the old ID can still come from history. |
| Two enumerations have different cuts | Separate invocation markers and per-enumeration chunk intervals preserve their different answers. |
| Enumeration pauses during history traversal | Its marker is reinserted at the remaining position, preserving the history still needed. |

These rows follow the initialization, table and history predicates above; they are proposed executable oracles, not a concurrency proof. Interval wrap, every publication state and weak-memory portability are not closed by this pass.

The history list is also an ownership protocol. A traversal temporarily removes its marker. It can detach and reclaim a leading deleted-entry prefix only until it encounters another invocation marker. If its allowance expires, it reinserts its own marker where it stopped. Normal reclamation frees the detached list **after releasing the table lock**. This prevents one completed observer from erasing another observer's evidence. [History traversal and detach][S07]

Abandonment takes a different path. `cleanup_ptab_list_bif_data` frees the continuation's arrays; if its marker has a predecessor, it unlinks just that marker. If it owns the head, it walks and frees the whole prefix through the next invocation marker **while holding the table write lock**. There is no resumable budget in that destructor loop. An old suspended observer plus substantial process churn can therefore produce a large cleanup workload. That is a source-derived workload hypothesis, not a measured pause or evidence of an upstream defect. [Abandoned-state cleanup][S08]

## 2. Where enumeration is budgeted, and where the budget stops

The normal snapshot engine uses 1,000-slot table chunks, 25 inspected slots per reduction, ten inspected deleted records per reduction, an extra reduction per reclaimed deleted record, a maximum 40-reduction history pass, and 75 result-list cells per reduction. The result-building phase allocates heap space and can shrink the temporary ID array between resumptions. [Constants][S09] [Table charging][S05] [History charging][S07] [Result building][S10]

These constants are accounting choices, not wall-clock guarantees:

- Initialization allocates an array proportional to the sampled population while holding the write lock. Allocator work is not made incremental by later scan charging. [Initialization][S03]
- The history loop charges deleted records, but crossing another invocation marker does not increment its work counters. Many active enumeration markers are a distinct workload from many deleted records. [Marker branch and charging][S07]
- Result allocation/reallocation, the abandoned-history destructor and allocator internals are not covered by a proof that every machine-time interval is bounded. [Result building][S10] [Destructor][S08]

Bream should measure scan work, lock hold time, allocation and reclamation separately. Replacing BEAM terms with Rust records does not resolve any uninterruptible allocation or large destructor on its own.

The newer iterator is a different contract, not a cheaper implementation of the same snapshot. Its public guarantee covers a process alive from iterator creation through the final `none`; transient processes need not form a set that existed at any single instant. The Erlang wrapper consumes a cached chunk before requesting another. The C path scans a bounded range based on remaining reductions, copies IDs, and uses entry/exit memory barriers without the snapshot history machinery. An ID cached in an earlier chunk may already be dead when returned to the caller. [Iterator contract and wrapper][S11] [Iterator C path][S12]

The C iterator's unprotected-looking pointer load is **not** evidence that a Rust implementation may read freed slots or dereference an arbitrary atomic pointer. Its safety must be considered with ERTS process-table retirement, covered separately in report 27. No Rust guard or epoch implementation has been established here.

## 3. Trace-session destruction has several completion boundaries

The documented session handle keeps the session alive; garbage collection of the dropped handle initiates cleanup. A weak session reference does not itself hold that lifetime. Destruction removes settings but does not retract trace messages already sent. [Public handle and destruction][S13] [Strong/weak types][S14]

The runtime gives those statements a phased implementation:

| Phase | Traced state and ownership |
| --- | --- |
| Active | Session is `ALIVE`; tracees may contain session references. |
| Explicit destroy starts | Acquire code-modification permission, resolve/retain the session, change `ALIVE` to `CLEARING`, unlink it from the global session list and clear default tracers/patterns. Suspend and retain the requesting process while code barriers run. |
| Last owning reference disappears | Magic-binary destructor schedules auxiliary destruction work and refuses immediate binary freeing. The auxiliary job obtains code-modification permission and enters the same clearing/barrier path without a requesting process. |
| Breakpoint barrier completes | Put the session on the cleaner wait list. Resume the explicit caller if it has not exited; release its retained process record. The cleaner still has work. |
| Cleaner round completes | Assert no breakpoint/tracee references remain, set `DEAD`, then release the explicit-operation hold or free a zero-reference session. Other owning references can delay final binary reclamation. |

[Explicit and auxiliary destroy][S15] [Global unlink and clearing][S16] [Destructor/final free][S17] [Barrier and caller handling][S18] [Cleaner finalization][S19]

Thus return from explicit destroy need not mean that every tracee node or the session allocation has been reclaimed. The observable disabling and code barrier precede final cleaner retirement. A caller dying while suspended must not cancel the cleanup obligation or be resumed as application work. Already-sent messages remain a separate lifetime. These are closely related to Bream's distinction between stopped computation, unaddressability and later resource reclamation, but do not prescribe an identical barrier implementation.

`new_tracer_ref` stores a session pointer in a tracee's linked list and increments only a debug bookkeeping count; it does not take the session magic binary's strong reference. Clearing zeroes the flags/session pointer before unlinking and freeing the reference node. The cleaner protocol is consequently part of the safety of those non-owning links. A blanket `Arc<Session>` on each tracee would change the ownership topology: active tracees would hold the session alive after public handles disappeared. Conversely, simply replacing those links with raw pointers would omit the retirement protocol. [Tracee-reference creation/removal][S20]

“Last public handle” is not itself an exact reclamation instant: some runtime work also takes strong session holds, including queued system-monitor events below. The important requirement is to distinguish user ownership, temporary work ownership and tracee attachment, rather than assume they all have the same meaning.

## 4. How the cleaner survives process death

The boot-time `erts_trace_cleaner` process calls **`processes/0`**, not the weak iterator. It sends cleanup requests up to `outstanding_system_requests_limit`, counts successful sends, waits for PID acknowledgments, then visits ports. Only after all accepted process requests have acknowledged and ports are handled does it call `check` again to retire the previous session batch. This limit bounds outstanding requests; it is not a cap on process mailboxes or total cleanup work. [Cleaner loop][S21]

The process request is a runtime RPC with generic reply generation disabled. If signal enqueue fails because the target is unavailable, the sender frees the RPC and reports failure; the cleaner adds no outstanding request. If accepted, `handle_trace_clean_rpc` scans all sessions in the cleaner batch, finds each target's matching tracer reference, obtains its minor locks when necessary, clears/removes those nodes, and sends the explicit PID acknowledgment. Both live and exiting signal dispatchers invoke the RPC handler. [Request/ack helper][S22] [RPC enqueue failure][S23] [Live dispatch][S24] [Exiting dispatch][S25]

Process exit separately removes references to non-alive sessions before process-table deletion. At the end of exit it emits any final scheduler trace, flushes trace messages and deletes all remaining trace references. These two sites matter: an exiting process cannot simply vanish from enumeration with an uncleared dead-session pointer, and a live-session attachment is eventually released by process teardown. [Before table deletion][S26] [Final trace cleanup][S27]

The resulting local reasoning is narrower than a full proof: the accepted-request path must acknowledge even if ordinary application execution has stopped; the failed-send path must not wait for an acknowledgment; and removing old sessions must not remove newly attached different sessions. This pass has not proven all spawn-inheritance, code-barrier or dirty-scheduler interleavings that prevent fresh old-session attachments during a cleaning round.

There is another accounting opportunity. The cleanup RPC loops over every clearing session and calls `get_tracer_ref`, which linearly searches the target's reference list; removal searches again and recomputes trace flags. The helper finally reports `*reds = 0`. Generic signal dispatch still charges its base work, so this is **not** a claim that the entire RPC is free. It does mean this callback reports no variable cost for its session/ref traversal and has no inner continuation. [Cleanup helper][S22] [Lookup/removal][S20] [RPC accounting][S28] [Base signal charge][S29]

## 5. A dead tracer is not the same event as a destroyed session

The default native tracer checks whether its destination process/port remains alive. For `trace_status`, a dead destination produces `remove`; for an ordinary event it produces `discard`. ERTS physically removes the tracee reference only when checking the tracee itself or a port on a normal scheduler; other contexts return disabled without performing that removal there. This is a lazy validation/removal path, not evidence that every tracer death synchronously destroys its session and every tracee attachment. Custom tracer callbacks are native calls and require a separate admission/budget analysis in Rust. [Default tracer][S30] [Callback and removal context][S31]

A second, distinct observation path retains more ownership. A queued system message stores its payload fragment and increments the associated session reference, if any. The dispatcher delivers/transfers the fragment or frees it on failure; when it later frees the processed queue node it drops that session reference. On failed system-monitor delivery it clears the monitor configuration only if the failed receiver still matches the current receiver, or is `NIL`. A stale failure must not disable a replacement receiver. This trace follows system-monitor/system-message routing, **not every ordinary trace-event delivery path**. [Queue ownership][S32] [Delivery/drop][S33] [Reference release][S34] [Failure guard][S35]

## 6. Tests inspected and remaining evidence gaps

No tests were run in this pass. The following existing tests make useful bounded oracles:

- `do_processes_bif_test` arranges process churn, priorities and forced reductions, compares sorted enumeration against a debug snapshot, and invokes kill-during-trap tests. The latter retries if its attempt failed to kill at the intended point; it is not a deterministic schedule proof. [Snapshot/churn and caller death][T01]
- `processes_term_proc_list_test` suspends callers in `ptab_list_continue`, alternates process death with new enumerators, resumes them out of order, then kills suspended enumerators and checks retained-list block counts. This directly targets marker ownership and abandonment. [Continuation setup][T02] [Resume order][T03] [Abandon order][T04]
- `trace_session_SUITE:destroy` covers explicit destruction and dropping the strong handle followed by garbage collection, then checks process/port/function/send/receive settings through the weak ID. It does not assert a bound on physical reclamation. [Session destruction test][T05]
- `trace_SUITE:tracer_die` checks replacement tracing after old tracers die; `trace_call_time_SUITE:dead_tracer` checks cessation of recording and that newly spawned tracers do not inherit stale tracing state. [Replacement test][T06] [Call-time/dead-tracer test][T07]

Unclosed branches include interval wrap, allocation failure policy during enumeration, the complete table-retirement weak-memory argument, all trace attachment/inheritance races, custom tracer NIF execution and unloading, all port/dirty-scheduler cleanup interactions, and ordinary trace-delivery ordering. The cited loops establish finite source work for a finite structure; they do not establish latency percentiles or guarantees under allocator contention, worker starvation, or native non-returning callbacks.

## 7. Concrete Rust opportunities and performance experiments

Rust can give continuation phases, observer IDs and detached-history batches distinct private types, making accidental construction or consuming the wrong phase harder through a controlled safe interface. This is an API-design opportunity grounded in [Rust privacy][R01], not proof of the unsafe backend or scheduling. A compact record need contain only identity/interval metadata, with no reference to application memory. Conversely, an `Arc` explicitly owns its value until the last strong owner disappears; using it indiscriminately would change the lifetime contract. [Rust `Arc`][R02]

A snapshot guard's destructor can improve ordinary-path cleanup, but it cannot be the sole guarantee that mandatory runtime cleanup happens: safe Rust may forget a value, and destructors are not guaranteed to run. A candidate runtime-owned continuation registry would need its own owner-death/shutdown protocol. This remains an experiment, including what may intentionally leak if a host retains or forgets a public handle. [Rust `forget` contract][R03]

All five experiments are proposed, unmeasured, and architecture-neutral. First implement deterministic branch oracles; then compare identical supported semantics with CPU-hog and spawn/kill traffic present. Report allocation counts/bytes, peak retained metadata, per-turn work, table-lock hold times, throughput and latency distributions separately. Accept only repeatable gains without semantic failures or unacceptable fairness/reclamation regressions; thresholds require a recorded experiment baseline, not an invented time guarantee.

| ID | Baseline → treatment; hypothesis | Semantic guard and rejection condition |
| --- | --- | --- |
| **OBS-R01 — Compact snapshot records** | Linked/generic identity results plus intermediate copies → typed ID chunks and compact deletion records, possibly transferring an owned buffer. Hypothesis: fewer allocations and less result construction. Measure sparse and dense tables, concurrent snapshots and churn. | Preserve one-cut membership and incarnation identity; retain no application heap merely to return IDs. Reject replacing the snapshot with the weaker iterator or measuring a smaller result contract as a speedup. |
| **OBS-R02 — Bounded abandonment detach/reclamation** | Prefix freeing under the table lock → first locate/detach reclaimable history in bounded work, then runtime-owned incremental freeing outside the lock. Hypothesis: lower spawn/exit contention after killing an old enumerator. | Preserve every later observer's history and exactly one owner of each detached node. Finding the boundary must itself be bounded or indexed; moving only `free()` outside the lock does not remove a linear scan. Reject lost records, premature release, cleanup tied solely to the dead caller, or unbounded deferred accumulation. |
| **OBS-R03 — Charge every continuation traversal** | Charge deleted entries but skip marker cost; bulk allocation/shrink → explicit phase/cursor work accounting including markers, with alternative buffer growth/shrink strategies. Hypothesis: tighter turn-cost distribution under many observers. | Charge bookkeeping without changing membership, including zero remaining allowance and caller death at every phase. Allocator latency remains separate. Reject extra resumptions that reduce throughput without the intended fairness benefit. |
| **OBS-R04 — Indexed session cleanup** | For each clearing session, scan a tracee list and remove its node → compare a single pass classifying attachments, or a compact typed index, plus resumable cleanup. Hypothesis: avoid repeated searches at high session counts. | Keep public ownership separate from attachment and temporary work holds. Accepted requests must acknowledge after the required work; failed sends must not create a wait. Do not reclaim a session while attachments/queued work can access it. Reject blanket per-tracee strong ownership as equivalent semantics. |
| **OBS-R05 — Compact observation events** | Generic payload fragments plus queue-node ownership → typed owned metadata records, optionally batch transferred, for an explicitly supported initial event subset. Hypothesis: lower allocation/copy cost and smaller queues. | Preserve incarnation, event ordering where promised, receiver-change failure guards and ownership of already queued events. Observation must neither resume a dead application nor prolong its computation. Reject benchmarking reduced event contents, silently dropping messages, or assuming session destruction drains the receiver inbox. |

## 8. Proposed additional behavior cases

Ten refinements were reserved after checking the catalogue: `OBS-01.C09–C13` and `OBS-02.C07–C11`. Existing cases remain unchanged. **Later/reference** means the full API is not an initial deliverable; its associated ownership/scheduling hazards still inform initial implementation reviews.

| Case | Trigger and required outcome | Allowed variation / forbidden outcome; evidence |
| --- | --- | --- |
| **OBS-01.C09** | **Later/reference snapshot:** spawn/delete/reuse a slot before/after the cut and before/after its chunk scan. Return precisely the cut's IDs once. | Return order is not the oracle; a returned ID may now be dead. Forbid post-cut replacement inclusion or losing the pre-cut incarnation. [S03], [S05], [S07], [T01]. |
| **OBS-01.C10** | **Later/reference overlapping snapshots:** pause at least three observers, churn, resume and kill them in non-creation order. Preserve each survivor's history and reclaim only unneeded prefixes. | Reclamation can be deferred under tracked ownership. Forbid one observer consuming another's evidence or freeing a shared suffix. [S07], [S08], [T02], [T03], [T04]. |
| **OBS-01.C11** | **Later/reference abandoned continuation:** owner dies during initialization, table scan, history scan or result construction, with many deletion nodes and invocation markers. Release or transfer every cleanup obligation once. | No exact time bound is asserted; separately record lock and turn work. Forbid mandatory cleanup depending on executing application `Drop`, or declaring reduction charging covers allocator/destructor latency. [S08], [S09], [S10]. |
| **OBS-01.C12** | **Initial ownership guard:** an observer retains IDs/metadata while the target is killed and its slot reused. `Down` still follows stopped/unaddressable computation; future lookup validates the incarnation. | Historical metadata may survive; it grants neither a process lease nor access to reclaimed application state. Forbid retaining the application heap merely because an observation record is retained. [S01], [S04], [S26], [S27]; accepted Bream lifecycle contract. |
| **OBS-01.C13** | **Later/reference weak iterator:** a stable process spans the full traversal; other processes appear/disappear between scans and cached-item returns. Include the stable process and finish through empty chunks. | Transients may not form a coherent snapshot; cached IDs may be dead. Forbid advertising snapshot semantics or treating a yielded ID as a liveness guarantee. [S11], [S12]. |
| **OBS-02.C07** | **Later/reference session ownership:** retain only weak IDs/tracee attachments, then remove all strong public and temporary work holds. Initiate cleanup without requiring tracee death. | Cleanup may be asynchronous. Forbid making attachment ownership prevent automatic cleanup, or freeing before non-owning attachments are cleared. Test retained strong handles separately. [S13], [S17], [S19], [S20], [S32], [S34], [T05]. |
| **OBS-02.C08** | **Later/reference destroy phases:** destroy an active session; kill the requesting process while waiting for the barrier; destroy again using a retained valid handle. Cleanup remains owned and the second destroy reports inactive. | Already-sent events may remain; physical retirement can follow return. Forbid resuming the dead requester, conflating barrier completion with all reclamation, or draining unrelated receiver messages. [S13], [S15], [S18], [S19]. |
| **OBS-02.C09** | **Later/reference cleaner race:** target dies before enqueue, after accepted enqueue, or during cleanup; another session is attached meanwhile. Failed send adds no pending request; accepted request acknowledges after cleaning its batch, including on the exiting path. | New unrelated attachments survive. Forbid an acknowledgment debt for failed send, premature batch retirement, or unaccounted variable work presented as bounded. [S21], [S22], [S23], [S25], [S28], [S29]. |
| **OBS-02.C10** | **Later/reference tracer death:** default tracer dies between status check/event delivery; repeat from self/other/dirty contexts and replace the tracer. Disable/discard appropriately and remove references only through qualified contexts. | Physical removal may be lazy; session destruction is a separate operation. Forbid reusing the dead tracer's state for new tracing or assuming an ordinary event check immediately clears all refs. Dirty/custom-native execution remains an unclosed branch. [S30], [S31], [T06], [T07]. |
| **OBS-02.C11** | **Initial event-ownership guard; full system-monitor API later:** queue an owned event, destroy its observer session or replace/kill its receiver before delivery. Deliver/drop according to the supported routing contract and release payload/session ownership once. | A queued event may outlive logical observer disablement. Forbid stale receiver failure clearing replacement configuration, use after session retirement, or extending application execution to satisfy observation. [S32], [S33], [S34], [S35]. |

## Source anchors

Immutable links below use the pinned commit. Useful local entry points: [enumeration](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_ptab.c:746), [abandonment](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_ptab.c:795), [session retirement](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_bif_trace.c:535), [cleaner loop](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erts_trace_cleaner.erl:44), [observer references](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_trace.c:3680).

[S01]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L5580-L5600
[S02]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L746-L793
[S03]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L888-L954
[S04]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L644-L734
[S05]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L956-L1044
[S06]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L1102-L1135
[S07]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L1047-L1202
[S08]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L795-L884
[S09]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L42-L57
[S10]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L1205-L1273
[S11]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L5602-L5666
[S12]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L1458-L1539
[S13]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/kernel/src/trace.erl#L1403-L1455
[S14]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/kernel/src/trace.erl#L153-L165
[S15]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_trace.c#L1279-L1348
[S16]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_trace.c#L1351-L1380
[S17]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_trace.c#L1188-L1227
[S18]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_trace.c#L535-L601
[S19]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_trace.c#L605-L646
[S20]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L3621-L3743
[S21]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erts_trace_cleaner.erl#L29-L100
[S22]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_trace.c#L650-L725
[S23]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L3380-L3431
[S24]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6807-L6813
[S25]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7400-L7416
[S26]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14429-L14470
[S27]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14781-L14794
[S28]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L3563-L3621
[S29]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6963-L6971
[S30]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/nifs/common/erl_tracer_nif.c#L121-L149
[S31]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L3285-L3374
[S32]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L2432-L2464
[S33]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L2740-L2858
[S34]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L2683-L2693
[S35]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L2530-L2562
[T01]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L2897-L3032
[T02]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L4696-L4744
[T03]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L4745-L4775
[T04]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L4778-L4806
[T05]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/trace_session_SUITE.erl#L1636-L1718
[T06]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/trace_SUITE.erl#L1922-L1951
[T07]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/trace_call_time_SUITE.erl#L545-L583
[R01]: https://doc.rust-lang.org/reference/visibility-and-privacy.html
[R02]: https://doc.rust-lang.org/std/sync/struct.Arc.html
[R03]: https://doc.rust-lang.org/std/mem/fn.forget.html
