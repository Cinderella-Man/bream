# Messaging, selective receive and signal transport

**Status: planned, not implemented or qualified.** These four features turn the inspected ordinary-local branches into build work. They do not claim that every possible interleaving is already specified. All acceptance tests below are future work; no tests or experiments were executed while writing this plan.

Part of the [master build plan](../build-plan.md). Reference: Erlang/OTP `OTP-29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. [Mailbox source review](../research/12-beam-mailboxes-source.md) explains the implementation; [sixteen-case audit](../research/15-mailbox-edge-cases.md) separates guarantees from unresolved boundaries. Branch IDs are stable: add cases without renumbering existing ones.

**D** means documented reference behavior; **I** means an inspected implementation detail; **R** means multiple concurrent outcomes must be represented; **U** means unresolved research or Rust mapping. A passing Bream test must check both the permitted outcome and the forbidden observations, including ownership. A U case blocks qualification of the affected capability; it does not license an arbitrary choice.

First scope: normal-priority local PID messaging, growing mailboxes, approved owned payloads, immutable buffers, unified application/lifecycle envelope and selective receive. Priority messages, aliases, registered-name addressing and distribution are later features. No GenServer or supervisor framework is needed.

## MSG-01 — Typed envelope, sending and ownership

**Deliverable:** a checked send/receive boundary for typed protocol payloads and runtime notifications, with explicit payload ownership through publication, selection, receiver death and cleanup. Send must not wait for mailbox capacity or application consumption. Exact public signatures remain design work; BEAM's send expression returns its term, whereas moving a Rust payload requires an ownership-preserving mapping.

**Implementation tasks:** define admissible payload/envelope representations; write the send ownership transition table; distinguish process identity from typed send capability; implement ordinary and self-send paths with one publication protocol; integrate dead-recipient disposal and immutable-buffer reference transfer. Record which steps can suspend and how partially prepared messages are reclaimed. Treat public construction of lifecycle-shaped messages as a researched mapping question, not an assumed private-constructor decision.

| Branch | Trigger and reference outcome | Forbidden observations; acceptance test |
|---|---|---|
| **MSG-01.C01** | Valid local live PID, admitted payload: ordinary send makes a message eligible for delivery; it is not a consumption acknowledgment. **D/I**, [send][send], [enqueue][enqueue]. | No inline invocation, duplicate delivery or waiting for a receiver handler. Pause receiver; send several messages, resume and account for each. |
| **MSG-01.C02** | Already-dead PID, or death between lookup and publication: absence is not ordinary PID-send failure; a message may be dropped. **D/I/R**, [dead paths][dead]. | No unintended replacement delivery before documented full-ID reuse, or leaked/double-released moved payload. Routing after full-ID reuse follows LIFE-01.C08's identity contract; internal references remain lifetime-safe. Inject death at every owned preparation/publication transition; verify disposal counts. |
| **MSG-01.C03** | Self-send: enqueue and later select normally; BEAM can avoid copying its own heap term. **D/I**. | No recursive handler entry, ordering shortcut or surviving mutable borrow. Send self A/B with selective B receive, then obtain A; exercise immutable and owned payloads. |
| **MSG-01.C04** | Sender continues, exits or is killed after transferring payload; receiver outlives it. Shared immutable storage survives while another owner needs it. **I/U Rust mapping**. | No sender mutation of transferred data, premature buffer free or arbitrary shared mutable object. Compile-reject escapes; kill each holder in turn and verify final release exactly once. |
| **MSG-01.C05** | Wrong protocol, runtime instance, stale generation or handle surviving shutdown. **U Rust/embedding mapping**, coordinated with LIFE-01/EXEC-05. | No type confusion, stale-slot delivery or unsafe dereference. Compile-reject protocol mismatch; runtime fixtures cover instance/generation/shutdown errors after their contract is recorded. |
| **MSG-01.C06** | Ordinary application message, eligible trapped exit and active-monitor Down reach one mailbox. **D + accepted envelope shape**, [notification actions][notifications]. | No unrelated public queues that change selection/order; no untrappable kill represented only as selectable data. Mix all variants and select across them. |
| **MSG-01.C07** | Application constructs lifecycle-shaped content or supplies a reference to explicit flush. BEAM tuples are forgeable; Bream constructor/matching policy remains **U**. | Do not silently equate a payload lookalike with a runtime signal, or silently remove BEAM-observable behavior. Execute the synthetic-Down/active-monitor flush fixture from the [lifecycle audit](../research/16-lifecycle-edge-cases.md); record representational equivalence or explicit necessary departure before API approval. |
| **MSG-01.C08** | Payload preparation/allocation fails or is interrupted before publication. **U** tied to admitted operations and failure classes. | No half-visible payload, lost owner, or claimed process-local recovery from host-global OOM. Enumerate failure points; inject recoverable failures and stop requests only where the qualified allocator/operation model supports them. |
| **MSG-01.C09** | **Send option boundary**. Deeper source refinement; [signal/timer report](../research/19-process-signals-timers-deep-dive.md). Conditional options/optimizations apply only when admitted. | Valid/invalid/improper option lists; local dead/live PID with nosuspend/noconnect; remote/port only in later transport profile. No bounded-mailbox interpretation. |

**Dependencies:** LANG-01/02 admission and types; LIFE-01 identity; LIFE-02/04 notification production; EXEC-03 owned cleanup/buffers and EXEC-04/05 failure/shutdown. C07 shares a blocking mapping decision with lifecycle flush cases. A simple FIFO prototype alone does not finish this feature.

**Performance-source refinements:** [report 34](../research/34-message-allocation-and-copy-costs.md). All remain planned. Reference-specific representations and optional/later capabilities are conditional; the Rust implementation must preserve the applicable ownership and observable behavior, not copy the storage layout.

| Case | Branch, required treatment and acceptance evidence |
|---|---|
| MSG-01.C10 | Self-send with/without sequence tracing and with on-/off-heap mailbox mode. Preserve identical payload and selective ordering despite different preparation paths; retain enabled trace behavior. Check snapshot immutability and lifetime after sender-local replacement. |
| MSG-01.C11 | MAIN try-lock fails, heap is volatile, GC is disabled, or free words equal requested size. Fragment fallback must preserve delivery/ownership without assuming synchronous receiver GC or waiting for its application code. Force each reference branch; qualify corresponding Rust path only if implemented. |
| MSG-01.C12 | Zero-size message reference versus combined fragment versus separate fragment, followed by selection, discard or receiver death. Release the right allocation/list exactly once; adopted receiver payload survives queue-node removal. Instrument allocation owners instead of inferring them from payload size. |

## MSG-02 — Selective receive, guards and scan state

**Deliverable:** generated matching over the unified envelope, preserving unmatched messages and automatically scheduling input-sized scans. The runtime owns the receive cursor; user code does not call yield. Matching borrows approved payloads until selection transfers ownership to the process.

**Implementation tasks:** specify message-major clause evaluation; lower supported patterns/guards into admitted operations; make failed matching and large pattern work resumable; implement reset paths for match, timeout and exceptions; remove the selected node before entering its body. Begin with linear search. Fresh-reference markers are an optional optimization only after an equivalence check proves their safety.

| Branch | Trigger and reference outcome | Forbidden observations; acceptance test |
|---|---|---|
| **MSG-02.C01** | Multiple clauses/messages match: oldest matching message wins; within it, first successful clause wins. **D**, [receive][receive]. | No clause-major scan. Queue an older clause-two match and newer clause-one match; assert older selection and preserved remainder. |
| **MSG-02.C02** | Pattern matches but guard is false or a permitted guard expression has invalid arguments: reject that guard, try its alternatives/later clauses. **D**, [guards][guards]. | No consumed rejected message or accidental conversion of every Rust panic into guard failure. Reference fixtures cover false guards, failing guard BIFs and semicolon alternatives; qualify each Rust guard mapping. |
| **MSG-02.C03** | No message matches, with no finite timeout: retain all messages and wait, possibly indefinitely. **D**. | No discarded unmatched prefix or busy-spin. Wake with one matching message, then use different clauses to recover the original messages in order. |
| **MSG-02.C04** | Scan or approved pattern work exhausts its allowance: suspend and resume its state. **I**, [scan][scan]. | No user-written yields, lost cursor or monopolizing scan. Use a long unmatched prefix while a peer advances; count inspections and verify bounded resumable helper work separately. |
| **MSG-02.C05** | Successful selection or timeout completes this receive: reset ordinary cursor for subsequent receive. **I supporting D**, [reset][reset]. | No permanent skipped prefix. First receive selects a late message or times out; second must select an older previously rejected message. |
| **MSG-02.C06** | Invalid timeout raises on the wait path, then application catches it: next receive can still see prior messages. **I/test**, [regression][regression]. | No stale save pointer or leaked marker becoming user data. Port both literal and dynamic invalid-timeout regressions; include nested subsequent receives. |
| **MSG-02.C07** | Fresh-reference optimization is enabled, marker absent/retired or immediate Down arrives. **I/U full marker lifecycle**. | No skipped eligible response or mistaken freshness of an arbitrary integer. Keep optimization disabled initially; later compare complete observable traces with unoptimized selection. |
| **MSG-02.C08** | Matching attempts an opaque callback, unbounded operator or side effect outside the admitted guard surface. **U Rust admission**. | No safe API escape that defeats preemption/kill or relies on predicate invocation counts. Compile-reject unqualified operations; qualify transitive helpers before admission. |
| **MSG-02.C09** | Selected body raises/exits or is killed: selection already removed its message. **I**, [lowering][lowering]. | No implicit message rollback/re-delivery or double cleanup. Raise immediately after selection; account for selected payload, retained mailbox and lifecycle outcome independently. |

**Dependencies:** MSG-01 envelope ownership; LANG-02/03 generated operations and suspension; EXEC-01/02 scheduling/accounting; EXEC-03 cleanup; lifecycle exception/exit representation. Arbitrary `Fn(&M) -> bool` is not a qualified matching surface merely because it is safe Rust.

## MSG-03 — Receive timeout and wakeup protocol

**Deliverable:** one receive-state machine joining scan exhaustion, wait registration, publication, timeout, cancellation and death. Timer expiration is runtime state, not an application mailbox message. Define receive generations so stale events cannot complete a different receive.

**Implementation tasks:** first finish reference interleaving analysis; record transition and ownership invariants; then implement a lock-based baseline or another reviewed publication/wait handshake. Separate timeout-expression evaluation, scan progress and timer arming. Expose deterministic test hooks around each transition; do not infer correctness from stress alone.

| Branch | Trigger and reference outcome | Forbidden observations; acceptance test |
|---|---|---|
| **MSG-03.C01** | `after 0`: select an available match or run timeout after necessary scanning. **D/I**, [timeout][timeout]. | No constant-time emptiness substitution or zero-latency promise. Exercise empty queue, early/late match and huge unmatched prefix with peer progress. |
| **MSG-03.C02** | `infinity`: wait indefinitely for a match; signals can still stop the process. **D/I**. | No overflow into finite expiry or inability to kill. Leave unmatched backlog, deliver match; separately kill a waiting process. |
| **MSG-03.C03** | Finite wait awakened by unmatched arrivals: existing timer is not restarted. Initial scan precedes arming in inspected execution. **I**, [wait][wait]. | No indefinitely sliding timeout or invented entry-time hard deadline. Flood unmatched arrivals; inspect arm count and subsequent timeout/reset behavior. |
| **MSG-03.C04** | Timeout expression errors, or value is outside `0..4294967295`/`infinity`: evaluation and wait-path validation are distinct. **D/I/U optimized variants**. | No unsupported claim that malformed values always error before queued matches. Reference fixtures cover effectful expressions, invalid literal/dynamic values, min/max and queued matches; settle Rust error mapping afterward. |
| **MSG-03.C05** | Producer publishes around the final empty scan and sleep registration. **I/U full interleavings**, [handoff][handoff]. | Never asleep with eligible work and no pending wakeup. Systematically pause producer/consumer before and after publication, lock release and ACTIVE transition. |
| **MSG-03.C06** | Match/arrival races timer expiry and scheduler continuation. **R/U**, [timer CAS][timer], [continuation][continuation]. | No chosen universal timestamp tie winner, double completion or lost selected payload. Derive allowed traces from the full reference protocol; enumerate bounded interleavings and retain both outcomes only where justified. |
| **MSG-03.C07** | Previous receive completed, timer cancellation/expiry arrives late; next receive or incarnation exists. **U full transition trace**. | No stale timeout completing new receive, stale wake resurrecting dead identity or timer use-after-free. Reuse slots and receive generations in controlled cancellation/death races. |
| **MSG-03.C08** | Direct kill races waiting, incoming message or timer. **D/I/R**: once exiting, no further user execution; earlier delivery does not require consumption. | No kill needing a matching clause or post-death body execution. Inject kill at each transition; assert stop separately from tracked reclamation completion. |
| **MSG-03.C09** | Receive has no message clauses, only `after`: suspend without consuming mailbox contents. **D**, [timer-only receive][timer-only]. | No accidental dequeue on wakeup. Send during the wait, then receive retained messages afterward; separately kill the waiting process. |
| **MSG-03.C10** | **Receive expiry refinement**. Deeper source refinement; [signal/timer report](../research/19-process-signals-timers-deep-dive.md). Conditional options/optimizations apply only when admitted. | Callback pointer CAS, already-TIMEDOUT cancellation, scheduler continuation, publication and kill. No timestamp winner, duplicate completion or stale-generation timeout. |

**Protocol refinements:** [Report 31](../research/31-receive-parking-wakeup-protocol.md). The [finite direct-parking model](../research/33-finite-ownership-interleavings.md) covers a restricted subset; the feature remains planned.

| Case | Controlled branch, oracle and scope |
|---|---|
| MSG-03.C11 | Timer expires before wait's sentinel check, between check/clear, and after `MSGQ` unlock. **Scope and exclusions:** Initial. Each branch retains or creates progress; forbid overwritten wakeup and duplicate timeout continuation. |
| MSG-03.C12 | Receive A matches/cancels, receive B arms; delay A's callback/physical cleanup, including timer-owner migration. **Scope and exclusions:** Identity/cancellation initial; migration conditional. Forbid A timing out B, duplicate cleanup or restarting A's timeout on unmatched traffic. |
| MSG-03.C13 | Matching input, unmatched suffix and expiry cross a scheduling boundary; compare zero/infinity/invalid timeout entry and subsequent receive. **Scope and exclusions:** Initial ordinary mailbox; priority/marker variants conditional. Record allowed outcomes by transition, forbid lost retained messages or an invented wall-clock tie-breaker. |

**Dependencies:** MSG-02 scan state; EXEC-01 wait/wake, EXEC-03 cleanup and EXEC-05 shutdown; LIFE-05 kill. REF-01/02 must resolve C04/C06/C07 before calling their behavior qualified. Timer-range/platform rounding also needs reference qualification, without an invented wall-clock guarantee.

## MSG-04 — Internal signals, ordering and work budgets

**Deliverable:** signal transport separate from application consumption, while preserving ordinary mailbox order for signals that become messages. ERTS's physical queue layout is evidence; Bream need not duplicate it to preserve observations.

**Implementation tasks:** define ordered send/publication boundaries; integrate link/monitor/exit traffic with ordinary messages; charge internal work; make large signal/queue cleanup resumable; establish ownership of pending, published, converted and abandoned entries. Start with one worker; qualify cross-worker transport separately under EXEC-07.

| Branch | Trigger and reference outcome | Forbidden observations; acceptance test |
|---|---|---|
| **MSG-04.C01** | Same sender/destination: message → exit → message preserves signal order. **D**, [order][order]. | No later message consumed beyond an effective earlier fatal exit; earlier messages need not all be consumed. Port `xm_sig_order`, including trapped-exit conversion. |
| **MSG-04.C02** | Independent senders/services race: only applicable per-sender order is fixed. **D/R**. | No global total-order claim. Generate interleavings and validate each sender subsequence while permitting justified cross-sender permutations. |
| **MSG-04.C03** | Eligible trapped exit/active-monitor Down converts to ordinary mailbox item; other signals act internally or are dropped. **D/I**. | No notification fabrication for inactive relationships or notification overtaking earlier ordinary items. Coordinate lifecycle race fixtures with a combined envelope trace. |
| **MSG-04.C04** | Internal signal burst or expensive conversion consumes allowance. **I**, [accounting][accounting]. | No uncharged housekeeping hog or claim that a nominal reduction maximum is exact latency. Measure work/peer progress; test suspension between qualified operations and document indivisible costs. |
| **MSG-04.C05** | Optional batching/buffers/flushes coexist with direct enqueue. **I/U alternative transport**. | No same-sender reorder or overwritten wakeup. Keep optimizations off until equivalence and boundary-interleaving fixtures pass for every physical path. |
| **MSG-04.C06** | Growing ordinary mailbox exceeds usual workload; receiver remains slow or dead. **Accepted policy/U resource limits**. | No hidden capacity wait or silent overflow loss for a live accepted message. Track queue growth, then kill/drain under explicit qualified failure policy; global OOM is not promised recoverable. |
| **MSG-04.C07** | Kill/exit during preparation, publication, conversion, selection or bulk teardown. **U ownership integration**. | No orphaned/double-freed node, payload or buffer; no cleanup user code defeating stop. Inject each transition; assert dead identity/Down separately from eventual tracked cleanup and peer progress. |
| **MSG-04.C08** | **Buffer transitions**. Deeper source refinement; [signal/timer report](../research/19-process-signals-timers-deep-dive.md). Conditional options/optimizations apply only when admitted. | Pending monitor followed by direct/buffered messages; install/flush/uninstall races; first nonempty publication versus sleep. No sender reordering or lost wakeup. |

**Protocol refinements:** [Report 30](../research/30-buffered-signal-publication-protocol.md). These remain planned; internal representations and conditional optimizations require their own qualification.

| Case | Trigger, required outcome and remaining obligation |
|---|---|
| MSG-04.C09 | Sender pauses after loading old array; receiver unpublishes it. Borrow/ref keeps allocation and mutexes valid until reader releases protection. **Forbidden:** acquiring a destroyed mutex or unprotected ref increment. |
| MSG-04.C10 | Race append with FREE and individual slot closure. Winning append is collected into exit ownership; losing append retains caller ownership for fallback. **Forbidden:** dropped ownership, duplicate cleanup or post-death application execution. |
| MSG-04.C11 | Pause publication around summary writes/bitmap exchange during flush. Selected work is collected or remains discoverable with notification. **Forbidden:** permanently invisible queued signal or double concatenation. |
| MSG-04.C12 | First buffered arrival races empty scan and sleep while MSGQ is held. Notification survives wait commitment without holding the slot mutex across MSGQ wait. **Forbidden:** lost wakeup or lock-cycle deadlock. |
| MSG-04.C13 | Sequential same-sender traffic alternates buffered/direct paths and ordinary/control types. Preserve signal order; allow unrelated-sender permutations. **Forbidden:** fallback overtaking its sender's accepted buffered prefix. |
| MSG-04.C14 | Pending monitor is prepended to message; receiver exits before fallback admission. Restore/resolve monitor ownership separately from discarded message payload. **Forbidden:** orphan monitor, double DOWN caused by ownership duplication, or stale pending pointer. |
| MSG-04.C15 | Unmanaged holder outlives baseline grace-period release; non-normal deinstaller dispatches cleanup to normal scheduler. Retain array through dispatch and every holder; final free once. **Forbidden:** baseline release before protected managed borrows finish or array ref mistaken for Process protection. |
| MSG-04.C16 | Live receiver adapts off/on or switches queue mode while sender caches old array. Old slot rejects after closure; transferred content remains ordered across fallback/new array. **Forbidden:** old-array reuse, duplicated stale slot contents or lost signals. |
| MSG-04.C17 | Pause stale control sender until receiver's deinstallation completed, then allow fallback installation checks in release/DEBUG configurations. **Unresolved oracle:** determine reachability and ownership of any replacement array. **Forbidden in a qualified implementation:** untracked array/signal leak, append after final cleanup without an owner, or false reclamation completion. |

**Dependencies:** MSG-01–03, LIFE-02–05, EXEC-01–04 and later EXEC-07. Signal-budget constants and three-queue organization are not prescribed algorithms or performance guarantees.

## Integration and qualification gate

Dependencies are defined in [admission and conformance](admission-and-conformance.md), [lifecycle](lifecycle.md) and [execution/resources](execution-and-resources.md). REF-01 records each branch's contract/source/test evidence; REF-02 runs exact-version reference fixtures and Bream comparisons. APP-01 then exercises these primitives in raw room/session traffic. MEASURE-01 measures scheduling delay, scan work, kill-to-stop and cleanup separately.

For every branch, retain distinct evidence fields: source inspected, reference executed, Bream executed, bounded interleavings explored and unresolved mapping. Completion requires no unresolved contradiction for the supported capability, permitted/forbidden observations checked, and owned-resource accounting across failures. Passing ordinary traffic alone does not qualify the race or failure branches.

[send]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/expressions.md#L647-L666
[enqueue]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L345-L359
[dead]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.c#L405-L426
[notifications]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L288-L317
[receive]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/expressions.md#L680-L713
[guards]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/expressions.md#L2331-L2334
[scan]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/emu/msg_instrs.tab#L44-L112
[reset]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/beam_jit_common.cpp#L1167-L1255
[regression]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/receive_SUITE.erl#L694-L757
[lowering]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/compiler/src/v3_core.erl#L4337-L4406
[timeout]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/expressions.md#L729-L763
[timer-only]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/expressions.md#L783-L794
[wait]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/beam_jit_common.cpp#L1208-L1239
[handoff]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7771-L7798
[timer]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L691-L706
[continuation]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10451-L10464
[order]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L495-L512
[accounting]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6968-L6971
