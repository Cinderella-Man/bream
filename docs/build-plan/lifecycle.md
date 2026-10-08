# Lifecycle feature and branch plan

**Status: planned/unimplemented throughout.** No implementation, oracle execution, or test pass is claimed. This inventory specifies work toward native Rust processes with automatic scheduling, owned state/messages, shared runtime-managed immutable bytes, a typed unified mailbox, BEAM-style lifecycle behavior, and execution stopped before DOWN. It is an extensible case catalogue, not proof that every possible interleaving has been enumerated.

[Master plan](../build-plan.md) · [Messaging](messaging.md) · [Execution/resources](execution-and-resources.md) · [Admission/conformance](admission-and-conformance.md).

Evidence: pinned OTP 29.1.1, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`; [source trace](../research/13-beam-lifecycle-source.md) and [18-case audit](../research/16-lifecycle-edge-cases.md), including its absent-link correction and synthetic-flush qualification. Known BEAM rules are defaults, not questions for the user. **Unresolved** below means an explicit evidence or Rust mapping gate, not permission to select convenient behavior.

Each case ID is also its future test identifier. Expand cases across applicable axes: self/other; live/exiting/dead/reused identity; trapping/untrapped; direct/linked signal; normal/kill/killed/other reason; absent/active/cancelling relationship; queued/in-flight/consumed notification; running/waiting/suspended process. Race tests must encode permitted sets plus causal ordering, not one observed schedule.

## LIFE-01 — Spawn and incarnation identity

**Implementation:** process registry with safe record lifetime, fresh mailbox/execution state, atomic initial relationship installation, validated construction and rollback. Typed message addresses refer to lifecycle identities independently of protocol payloads; dropping an address does not implicitly terminate a process.

**Dependencies:** production foundation LANG-01/02/03 and EXEC-01; shared signal/ownership specification with MSG-01/04 and LIFE-02/03. Test setup REF-01/02. These coupled specifications do not require all public APIs implemented before the registry can be tested.

| Case | Trigger/preconditions | Required outcome, forbidden result, and planned check |
|---|---|---|
| LIFE-01.C01 | Ordinary valid spawn | Fresh incarnation, empty mailbox, independent resident state. Assert no inherited queued messages or stale relationships. |
| LIFE-01.C02 | Spawn returns while child runs/exits | Return is construction success, not readiness or continued liveness. Test child before/after parent resumes. |
| LIFE-01.C03 | Spawn-monitor; first child action exits | Relationship exists before user execution; actual child reason reaches uncancelled observer, never registration-gap noproc. Test normal and abnormal first actions. |
| LIFE-01.C04 | Spawn-link; immediate child death | Apply LIFE-04 rules, including possible parent termination. No gap between publication and initial relationship protection. |
| LIFE-01.C05 | Combined link+monitor; stable trapping observer | Both installed atomically. **Unresolved oracle:** establish precise EXIT/DOWN ordering for supported variants; do not promise both consumed by a nontrapping dying parent. |
| LIFE-01.C06 | Invalid construction/options or process-capacity failure | Reject consistently; no runnable orphan or leaked partial relationship. Map BEAM badarg/system-limit separately; Rust error/owned-input return form unresolved. Inject every allocation/publication failure. |
| LIFE-01.C07 | Slot reused while old handle survives | Full identity validation rejects stale incarnation before documented full-ID reuse; no pointer lifetime race or delivery to replacement by slot alone. |
| LIFE-01.C08 | Generation counter exhausts/wraps | BEAM permits eventual PID reuse. Specify finite-ID behavior and safe reclamation first; test reduced-width model. Never equate generation equality with memory safety. |
| LIFE-01.C09 | Handles cross runtime instances | Define namespace/validation in Rust representation; no accidental lookup in an unrelated registry. Test equal slot/generation values across instances. |
| LIFE-01.C10 | Address clones dropped; parent dies | Address count alone does not kill child. Parent death affects child through installed links, not implicit ownership; monitor-only child remains independently scheduled. |

**Evidence/gaps:** [fresh state](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12722-L12749); [atomic relationships](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12908-L12945); [first scheduling](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13115-L13121). Pinned cleanup visits links before monitors ([phase order](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14545-L14584)), suggesting EXIT-before-DOWN in the ordinary stable local case; REF-01 must distinguish this source prediction from a universal API guarantee. Runtime namespace and generation representation remain design work, not new BEAM semantics.

**Rust ownership/source refinements:** [report 39](../research/39-process-table-and-spawn-publication.md). These cases are planned qualification obligations. BEAM representation-specific branches are conditional reference work; a Rust alternative must meet the supported observable and ownership contract. Language-probe results do not qualify Bream features.

| Case | Branch, required treatment and acceptance evidence |
|---|---|
| LIFE-01.C11 | Concurrent capacity reservation crosses the limit; a competing reservation rolls back or an exiting record returns its identity. Exactly one outcome per reservation, coherent free-token/count accounting, no application execution from an incompletely initialized child. Exercise both CAS rollback success and retry-then-proceed. Do not equate initial over-limit observation with final failure. |
| LIFE-01.C12 | Default free-ID consumption encounters holes, stalled return publication and cursor wrap; separately enable a probing allocator. No duplicate ownership or overwrite of a live slot; free tokens match vacant eligible slots after quiescence. Measure retry work and require a progress argument; legacy lock escalation is conditional reference coverage, not an initial Rust feature. |
| LIFE-01.C13 | Capacity rounds upward, reaches absolute maximum, or identity arithmetic reaches invalid/full-wrap encoding. Reconcile reserved sentinel with free pool and public count, reject invalid IDs, preserve index/generation mapping. Maximum-size interaction is source-unresolved; use a reduced model before a large reference run. Extends C08 without silently selecting Rust width/wrap policy. |
| LIFE-01.C14 | PID is enumerated while creator holds construction locks; send, monitor, inspect or kill races trace-lock release and remaining initialization. No uninitialized access, lost accepted input or application execution before initial relationship protection. Trace/enumeration visibility is not readiness; collect allowed race outcomes per operation. Full cross-operation oracle remains unexecuted. |
| LIFE-01.C15 | Body allocation fails before reservation; table full after body allocation; later heap/endpoint/optional-metadata initialization fails or unwinds. Distinguish reference's recoverable SYSTEM_LIMIT paths from fatal allocator paths. For each supported Rust recovery branch, release only initialized owners and reservations exactly once; no orphan endpoint. Public error/input-return/panic policy remains unselected. |
| LIFE-01.C16 | Reused full PID collides with an existing parent link during spawn-link, including priority metadata if supported. Existing versus rejected allocation ownership is explicit; forbid post-release access and duplicate priority accounting. Determine merge/replace/reject semantics via reduced model/reference work; source concern is not a demonstrated upstream failure. |
| LIFE-01.C17 | Typed address is erased/restored, copied across runtime instances or retained through slot reuse; a record borrow attempts to escape its guard. Compile-time protocol separation plus runtime namespace/incarnation validation; no safe escaped reference. Retained record does not retain application life; dropped/forgotten addresses cannot change stop semantics. |
| LIFE-01.C18 | Chunked slab grows or cold metadata first publishes while another operation reads, kills or retires the record; initialization loses its CAS. Stable valid storage under protection, single owner of winning metadata, one cleanup of losing candidates, tracked partial initialization. Compare actual allocation sizes with reporting; representation-specific subcases apply only if selected. |

## LIFE-02 — Monitor, demonitor, flush, and info

**Implementation:** unique observation references, independently owned endpoints, one-shot notification conversion, cancellation barrier at the observer, and mailbox-aware flush. Never use target PID alone as the subscription key.

**Dependencies:** production registry LIFE-01; MSG-01/02/04 for ordered envelope insertion and reference-scoped removal; EXEC-03 for endpoint/payload lifetime. Test setup REF-02 interleavings and allocation accounting.

| Case | Trigger/preconditions | Required outcome, forbidden result, and planned check |
|---|---|---|
| LIFE-02.C01 | Monitor self | Return reference; install no relationship or runtime DOWN. Check registry and mailbox. |
| LIFE-02.C02 | Repeated monitors of same target | Independent references, one outcome each unless cancelled; cancelling one leaves others. Reject accidental deduplication. |
| LIFE-02.C03 | Already-dead target | Reference plus asynchronous DOWN/noproc, not historical reason or synchronous link-style failure. |
| LIFE-02.C04 | Monitor races with death | Actual reason or noproc according to establishment timing; exactly one outcome per uncancelled relationship. Enumerate before/after installation. |
| LIFE-02.C05 | Default demonitor before DOWN materializes | After return, no future runtime DOWN enters mailbox for that reference, even if signal remains in flight. |
| LIFE-02.C06 | Default demonitor after DOWN queued | Existing notification remains. Test consumed versus unconsumed history separately. |
| LIFE-02.C07 | Flush with genuine queued DOWN | Remove corresponding notification and stop observation; preserve unrelated references/messages/order. No target cancellation. |
| LIFE-02.C08 | Info with active origin | True means removed; no prior/future runtime DOWN for that relationship. Not a target-liveness query. |
| LIFE-02.C09 | Info after removal/notification/consumption | False means absent; it does not prove a notification remains queued. Cover all three histories. |
| LIFE-02.C10 | Repeated default demonitor/unused local reference | Successful no-op; info returns false when absent. Typed-token representation may exclude arbitrary reference construction; record exact supported mapping. |
| LIFE-02.C11 | Info plus flush | Return reflects origin removal, not number of messages removed. Check active, queued, consumed, and repeated calls. |
| LIFE-02.C12 | Synthetic reference-bearing notification | [Pinned VM observations](../research/29-pinned-reference-observations.md) confirm active-origin flush skips the synthetic tuple while absent-origin flush scans. Typed look-alike constructibility remains unresolved and outside the restricted genuine-Down milestone. |
| LIFE-02.C13 | Wrong-owner/invalid/foreign reference | Preserve documented validity distinctions where exposed; never corrupt another observer's monitor. Enumerate representable errors versus compile-time exclusions. |
| LIFE-02.C14 | Observer exits; target concurrently dies | Remove observer-owned relationships and release both endpoint obligations exactly once; no retained dead observer. Test crossing demonitor/DOWN. |

**Protocol refinements:** [Report 32](../research/32-relationship-cancellation-protocol.md). These remain planned; internal representations and conditional optimizations require their own qualification.

| Case | Trigger, required outcome and remaining obligation |
|---|---|
| LIFE-02.C15 | MONITOR remains pending when canceled, including target disappearance. Preserve MONITOR-before-DEMONITOR; suppress resulting DOWN after origin removal; release both obligations. Model scheduler pending slot, not only receiver queue. |
| LIFE-02.C16 | In-flight immediate versus compound DOWN crosses cancellation. Origin-removal-first forbids later genuine notification; conversion-first permits retained DOWN without flush. Trace both representation/release paths and assert exactly-once destruction. |
| LIFE-02.C17 | Absent origin; two matching five-tuples and unrelated sentinel already queued. Flush removes first eligible match only, preserves remainder/order, returns false with info. Compare active-origin fast path separately; typed constructibility unresolved. |
| LIFE-02.C18 | Application sends a look-alike after cancellation/flush returns. It remains receivable; suppression applies to monitor-generated notification, not arbitrary later message. Use post-return sender handshake; forbid installing a reference blacklist. |

**Evidence/gaps:** [cancellation/flush/info contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L2024-L2103); [conditional flush dispatch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L639-L654); [DOWN conversion gate](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6367-L6378). C12 needs active-origin, documented-expanded-receive, and second-flush fixtures, also with info. The typed envelope's public construction and send rules remain unresolved: do not invent either permission or prohibition of lifecycle look-alikes. Flush scans participate in EXEC-02 work accounting.

## LIFE-03 — Link, unlink, and relink

**Implementation:** bidirectional failure relation with idempotent active setup, immediate local deactivation, and correlated unlink completion. ERTS's two-endpoint layout is evidence, not a required Rust data structure.

**Dependencies:** production LIFE-01, LIFE-04, MSG-04; ownership cleanup EXEC-03. Test setup deterministic signal delays in REF-02.

| Case | Trigger/preconditions | Required outcome, forbidden result, and planned check |
|---|---|---|
| LIFE-03.C01 | Link/unlink self | Successful no-op; no self-link record. |
| LIFE-03.C02 | Repeat existing active link | One relation, not counted subscriptions; return does not establish peer liveness. |
| LIFE-03.C03 | Local target absent at cheap check; caller untrapped | Catchable noproc error, not forced exit. Rust representation must distinguish this from signal death. |
| LIFE-03.C04 | Absent target; caller trapping | Linked noproc signal follows trap handling; no silent success without outcome. |
| LIFE-03.C05 | Target disappears during link setup | Depending on check/installation: immediate error or later linked reason/noproc. Preserve causal conditions, not arbitrary winner selection. |
| LIFE-03.C06 | Unlink absent/dead/already-unlinking peer | Successful no-op or completion of pending removal; no duplicate cleanup. |
| LIFE-03.C07 | Unlink races with linked exit | After return, no future effect from removed link unless relinked; already-queued trapped EXIT remains. |
| LIFE-03.C08 | Direct exit from same PID after unlink | Direct signal still applies; unlink must not become a sender blacklist or flush unrelated EXIT. |
| LIFE-03.C09 | Unlink, relink, delayed old acknowledgment | Old acknowledgment cannot erase new active relation. Do not assume relink suppresses every older in-flight death signal. |
| LIFE-03.C10 | Both endpoints concurrently link/unlink | Converge to causally valid consistent relation; test both permitted final states and all acknowledgments, not one scheduler winner. |
| LIFE-03.C11 | Peer enters cleanup during unlink | Finish acknowledgment/ownership protocol or absence handling; no stranded entry or waiter. |

**Protocol refinements:** [Report 32](../research/32-relationship-cancellation-protocol.md). These remain planned; internal representations and conditional optimizations require their own qualification.

| Case | Trigger, required outcome and remaining obligation |
|---|---|
| LIFE-03.C12 | ACK for unlink reaches a reactivated endpoint. Preserve the active relation. Use ordered peer barrier and subsequent linked death; instrument interleaving to prove ACK was pending at relink. |
| LIFE-03.C13 | Unlink i → relink → unlink j; ACK i arrives while j is pending. Old ACK must not erase j; ACK j can finish cleanup. Inject this exact sequence in protocol model and verify allocation accounting. |
| LIFE-03.C14 | Incoming LINK while own unlink remains pending, contrasted with own relink call. Incoming LINK cannot reactivate; own relink can. Enumerate crossed operations and drain ACKs before checking endpoint agreement. |
| LIFE-03.C15 | Linked EXIT meets inactive endpoint, with ACK still pending. Suppress notification/failure and remove endpoint; later ACK is harmless. Separate already-materialized EXIT and reactivated endpoint controls; no unconditional stale-EXIT filter. |
| LIFE-03.C16 | Peer begins exit with unlink pending; late incoming LINK arrives. Do not propagate historical reason through inactive old link; late LINK follows noproc handling. Qualify alive-check races and exercise both exiting dispatch and absent lookup. |

**Evidence/gaps:** [missing-peer exception and idempotence](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L3866-L3881); [unlink contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L6852-L6877); [acknowledgment identity](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6758-L6771). Qualify C05/C09 race sets with REF-01 traces; the sequential rules are already known. Rust catchable operation failure must not reuse forced-kill machinery.

## LIFE-04 — Exit reasons and trapping

**Implementation:** distinct internal direct/linked control operations; exit policy sampled on receipt; eligible EXIT notification insertion through the unified mailbox; immutable committed exit reason. Process-local exception representation needs LANG-02/EXEC-04 review.

**Dependencies:** production MSG-01/04, LIFE-01/03, LANG-03; final stop/cleanup LIFE-05. Tests can start with a pure transition table before native execution exists.

| Case | Trigger/preconditions | Required outcome, forbidden result, and planned check |
|---|---|---|
| LIFE-04.C01 | Process body returns normally | Normal termination; links/monitors see normal under their distinct rules. |
| LIFE-04.C02 | Unhandled process exception/fault | Single abnormal reason; owned reason payload survives notification delivery. Rust panic/error mapping remains explicit work. |
| LIFE-04.C03 | Normal direct-to-other or active linked exit | Untrapped: ignore. Trapped: EXIT/normal. Never mistake normal link death for mandatory peer termination. |
| LIFE-04.C04 | Other abnormal reason | Untrapped: terminate with reason. Trapped: EXIT message. Application receive is unnecessary for runtime signal action. |
| LIFE-04.C05 | Direct kill | Irreversible termination with killed, regardless of trapping/catch; no application-message implementation. |
| LIFE-04.C06 | Link-propagated literal kill/killed | Kill remains kill; trapped receiver gets EXIT, untrapped terminates. Directly killed target propagates killed normally. |
| LIFE-04.C07 | Trap flag changes around reception | Use reception-time policy. Enumerate before/after receipt, including already-materialized notification; no send-time snapshot. |
| LIFE-04.C08 | Linked signal after deactivation | Discard unless relationship was re-established under valid protocol; never apply direct-signal rules solely from reason. |
| LIFE-04.C09 | Self exit through exit/1-equivalent | Catchable process exception; only uncaught exception ends process. Trap-exit flag is not exception catching. |
| LIFE-04.C10 | Legacy exit(self(),normal), trapped/untrapped | Untrapped terminates normally; trapped receives notification. Preserve if exposing that operation; no silent alias to newer semantics. |
| LIFE-04.C11 | New exit_signal(self(),normal); self abnormal/kill | Normal untrapped ignored; other untrapped reasons terminate; eligible trapped reasons notify; direct kill terminates. Signal termination cannot be caught as exit/1 exception. |
| LIFE-04.C12 | Same sender sends message/exit/message | Preserve signal order; do not promise processing of messages after terminating signal. Different senders have no invented total order. |
| LIFE-04.C13 | Fresh child; setting trap flag repeatedly | Initially untrapped even if parent traps; setting flag returns previous value. Test both transitions and repeated same-value setting. |

**Evidence/gaps:** [exit decision matrix](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L657-L705); [kill and self-operation distinctions](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L2496-L2586); [signal order](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L507-L511); [fresh flags](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12443-L12555), [flag return value](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L2058-L2067). Exhaustively cover the decision-table cross-product, including ignored signals, rather than treating a successful normal-exit demo as exit conformance.

## LIFE-05 — Kill, death observation, and reclamation

**Implementation:** one-way live→exiting transition; disable future user execution, remove live identity, emit observation events, and complete separately tracked cleanup. Runtime work may remain after DOWN; admitted arbitrary native application work may not.

**Dependencies:** production LANG-03/EXEC-01/02 checkpoints, EXEC-03 resources, MSG-03/04 wakeups and control, LIFE-01/02/03. Host shutdown coordinates through EXEC-05. REF-02 supplies resource counts and controlled interleavings, not production introspection requirements.

| Case | Trigger/preconditions | Required outcome, forbidden result, and planned check |
|---|---|---|
| LIFE-05.C01 | Kill CPU loop/recursive computation | Automatic runtime intervention; no user-written yield/receive needed; continuation never resumes after exit commits. |
| LIFE-05.C02 | Kill waiting/suspended/timeout-racing process | Wake exit machinery without restoring application execution. Race receive timeout and mailbox arrival against stop. |
| LIFE-05.C03 | Kill inside admitted helper/allocation/scan | Reach qualified stop point; runtime ownership remains valid. Unsupported uninterruptible native escapes must not be admitted. |
| LIFE-05.C04 | Repeated kills; kill exiting/dead incarnation | At most one committed death per incarnation and one DOWN per monitor; no duplicate release, reason rewrite or resurrection. No unintended replacement-target kill before documented full-ID reuse; subsequent routing follows LIFE-01.C08. Internal stale work cannot affect a replacement regardless of ID reuse. |
| LIFE-05.C05 | Competing abnormal exits/kill/normal completion | One committed reason wins consistent with causal receipt/execution. **Unresolved oracle:** enumerate precise allowed sets; no universal “kill always wins.” |
| LIFE-05.C06 | Observer receives DOWN | Target application execution stopped and identity removed; no further admitted helper execution. Stronger than BEAM dirty-native exception. |
| LIFE-05.C07 | Reclamation completion with shared immutable buffers | Release dead process's ownership once; other holders remain valid. Completion does not imply globally freed buffer or reduced RSS. |
| LIFE-05.C08 | Outstanding runtime I/O owns memory | Keep required memory valid through completion/cancellation; user execution stays stopped. Never free based solely on DOWN. |
| LIFE-05.C09 | Large mailbox/relationships/timers at death | Incremental scheduled cleanup with retained cursor; unrelated runnable processes progress. Measure phase cost; no unsupported hard deadline. |
| LIFE-05.C10 | New monitor/link/unlink during exit cleanup | Reply/cancel according to lifecycle protocol, including late noproc and unlink acknowledgment; release every ownership-bearing signal. |
| LIFE-05.C11 | Cleanup failure/host shutdown during reclamation | Preserve committed death and memory safety; specify surfaced failure/accounting with EXEC-04/05. No hidden indefinite quarantine or false completion. |
| LIFE-05.C12 | Pause a protected reader after slot load; remove process and publish a replacement. Old record remains readable only within protection; full-ID mismatch rejects replacement. No use-after-free or mistaken routing before full-ID reuse. | [Lookup/reclamation protocol](../research/27-process-lookup-reclamation-protocol.md) supplies the ownership and memory-order obligations; controlled interleavings and Rust qualification remain future work. |
| LIFE-05.C13 | Lookup blocks acquiring a process lock while target commits exit/removal. On acquisition revalidate; never return an ordinary live handle for the dead record. Explicit exiting-record operations must still reject removal. | [Lookup/reclamation protocol](../research/27-process-lookup-reclamation-protocol.md) supplies the ownership and memory-order obligations; controlled interleavings and Rust qualification remain future work. |
| LIFE-05.C14 | An unmanaged reader upgrades temporary protection to a record ref before blocking. Retirement can progress after guard release, but record cannot free before ref release. Audit every success/failure/TRY/flag branch for balanced ownership. | [Lookup/reclamation protocol](../research/27-process-lookup-reclamation-protocol.md) supplies the ownership and memory-order obligations; controlled interleavings and Rust qualification remain future work. |
| LIFE-05.C15 | Two-process ordered locking temporarily releases the caller's final lock. Preserve both allocations, reacquire in order, and revalidate observations whose protection was lost; forbid deadlock and stale field access. | [Lookup/reclamation protocol](../research/27-process-lookup-reclamation-protocol.md) supplies the ownership and memory-order obligations; controlled interleavings and Rust qualification remain future work. |
| LIFE-05.C16 | Retain a record through logical death and ordinary body cleanup. Live lookup stays absent and heap access stays forbidden; final ref release frees record exactly once. Record ref must not masquerade as process-state ownership. | [Lookup/reclamation protocol](../research/27-process-lookup-reclamation-protocol.md) supplies the ownership and memory-order obligations; controlled interleavings and Rust qualification remain future work. |
| LIFE-05.C17 | Death leaves proxy, dirty-queue or obsolete ordinary queue ownership. Drain each owner exactly once without executing dead application code; no early free, duplicate release or replacement resurrection. Dirty-native reference behaviour is reference evidence, not admission permission. | [Lookup/reclamation protocol](../research/27-process-lookup-reclamation-protocol.md) supplies the ownership and memory-order obligations; controlled interleavings and Rust qualification remain future work. |
| LIFE-05.C18 | Pause unmanaged entry during counter-bucket switch and during upgrade-to-ref. Every accepted lookup has continuous protection; failed entry retries release the old bucket; no unprotected gap or permanent artificial delay. | [Lookup/reclamation protocol](../research/27-process-lookup-reclamation-protocol.md) supplies the ownership and memory-order obligations; controlled interleavings and Rust qualification remain future work. |
| LIFE-05.C19 | Delay a managed participant or an unmanaged guard across retirement. Defer reclamation safely and account for retained storage; resume permits progress. No false cleanup-complete notification or invented latency bound. | [Lookup/reclamation protocol](../research/27-process-lookup-reclamation-protocol.md) supplies the ownership and memory-order obligations; controlled interleavings and Rust qualification remain future work. |
| LIFE-05.C20 | Force small test-only generation/progress counters and allocator-address reuse. Distinguish slot reuse, full public-ID reuse and pointer-address reuse; stale internal ownership cannot target the replacement. Verify wrap predicates independently of steady-state tests. | [Lookup/reclamation protocol](../research/27-process-lookup-reclamation-protocol.md) supplies the ownership and memory-order obligations; controlled interleavings and Rust qualification remain future work. |

**Deeper evidence:** [Report 27](../research/27-process-lookup-reclamation-protocol.md) extends this group with protocol invariants and remaining limits.

**Evidence/gaps:** [exit transition](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13485-L13547); [cleanup continuation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14234-L14315); [later exits released during cleanup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7311-L7338); [live identity removal](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14420-L14469). BEAM final destruction is not universally budgeted; Bream needs its own EXEC-03 proof. Safe kill requires closure of admitted code/resources, not arbitrary stack destruction or arbitrary user Drop.

**System-task, code-lifetime and routing refinements:** [report 50](../research/50-system-task-dispatch-and-dirty-handoff.md). These are planned cases; source-only, executed reference and Rust compiler evidence remain separate. Optional naming, native jobs and code replacement do not become initial requirements.

| Case | Branch, required treatment and acceptance evidence |
|---|---|
| LIFE-05.C21 | At an admitted helper's completion gate, race committed death with result/exception/continuation publication and partial owned allocation transfer. Use dirty-native return only as reference lifetime evidence. Publishing a continuation after committed death, freeing still-owned partial allocations, or treating record retention as permission to resume. Bream must stop admitted application/helper execution before `Down`; broad host jobs remain unselected. |

## Completion gates and traceability

All 59 cases above are planned. REF-01 must resolve flagged source/documentation gaps with pinned-version oracles; REF-02 must compare permitted traces, assertions, ownership accounting, and forbidden outcomes. Add new IDs when research discovers branches; never rename an existing case to conceal a changed contract.

Before claiming lifecycle support: map every public operation/error to these cases; implement transition-model tests; add generated native fixtures and stress schedules; demonstrate no post-DOWN application progress; prove endpoint/buffer ownership across cancellation; measure cleanup fairness; and record unsupported forms explicitly. Compile failures may enforce unsupported Rust forms, but they are not runtime conformance passes.

Distribution, process aliases, priority notifications, ports/NIF monitors, registered-name APIs, spawn-request cancellation, and OTP behaviours are separate later feature inventories. They must not appear as silently supported variants of these local process primitives.
