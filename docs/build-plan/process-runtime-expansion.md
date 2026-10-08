# Process-runtime expansion: deeper branches and conditional features

**Status: planned; selected BEAM reference variants executed in [report29](../research/29-pinned-reference-observations.md). No Bream feature is implemented or qualified by those observations.** This extends the [master plan](../build-plan.md) from the deeper pinned-source review. Read the [surface inventory](../process-runtime-inventory.md) for inclusion/exclusion boundaries and review depth. The reference is OTP-29.1.1, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`.

These groups combine initial correctness refinements with later runtime capabilities and representation research. A case concerning an Erlang-specific GC policy, port, remote node or code purge is a reference/mapping obligation if that capability is proposed, not an automatic first-release requirement. Rust ownership and automatic scheduling must be qualified in their own model. Compression, cryptography and other bundled utility algorithms are excluded. No GenServer or supervisor implementation is added.

Each table records a branch plus the required observation or qualification task. The linked report supplies immutable source ranges, inspected tests and unresolved points. A case can cover several interleavings and is not a test-pass count. Qualification also requires the master plan's failure, ownership, forbidden-outcome and evidence fields. Unverified source predictions stay unresolved until a matching-version fixture establishes the relevant contract.

The following three additions extend existing feature tables instead of creating duplicate groups: **MSG-01.C09** (send options), **MSG-03.C10** (receive expiry) and **MSG-04.C08** (buffer transitions), in [messaging](messaging.md).

## Workstream index

| Feature | Focus | Applicability |
|---|---|---|
| EXEC-08 | Advanced scheduling and explicit suspension | Later priority/suspension APIs and G7 concurrency; exclusive claims and wakeup correctness already apply at G2. |
| MEM-01 | Owned memory, GC reference branches and hibernation | Initial ownership/accounting at G1–G2; GC knobs, allocation-mode controls and hibernation are representation research or later capabilities. |
| STATE-01 | Process-local dictionary and metadata | Owned-state invariants apply initially; a dynamic dictionary API remains an extension. |
| MSG-05 | Aliases and reply-address lifetime | Later typed addressing capability; ordinary PID send remains the first profile. |
| MSG-06 | Priority message reception | Later mailbox feature, separate from scheduler priorities. |
| MSG-07 | Receive-marker optimization | Optional optimization after correct unoptimized receive. |
| TIME-01 | General message timers and paused-timer identity | Later timer API; receive timeout refinements remain in MSG-03. Explicit suspension is separately conditional. |
| LIFE-06 | Expanded spawn options and request lifetime | Initial atomic-spawn refinements plus later asynchronous/remote request APIs. Each conditional case applies only when that capability is admitted. |
| LIFE-07 | Relationship options, references and exit continuations | Initial relationship/exit refinements plus conditional alias, tag, priority and remote variants. |
| NAME-01 | Local name registration lifetime | Later addressing/registry capability, independent of supervision. |
| OBS-01 | Ordered inspection and process enumeration | Minimal correct observation initially; broad per-item compatibility and enumeration are extensions. |
| OBS-02 | Process diagnostics, sensitivity and event lifetime | Later diagnostic APIs; basic counters continue in EXEC-06. |
| IO-01 | Process I/O routing and native resource boundary | Runtime-aware I/O is accepted but follows the local process slice. Native-call admission remains an initial safety obligation. |
| LANG-04 | Catchable failures and lifecycle handoff | Initial mapping obligation alongside checked code and exit semantics. |
| CODE-01 | Executable-code lifetime and purge boundary | Later research only; static linked Rust code may exclude replacement entirely. |
| SIG-01 | Control dispatch through process death | Initial exhaustive handling for every admitted internal control variant; unsupported extension variants remain out of the implementation. |
| RES-01 | Process-owned external state and ownership transfer | Reference and later integration work. Shared ETS-like mutable state and port APIs are outside the accepted initial state model. |

## Next experiment batches

The first bounded reference run is recorded in [report29](../research/29-pinned-reference-observations.md): part of batches A/F plus the optimized-marker branch of D. Those batches remain incomplete. Run remaining investigations with raw traces and an exact executable identity. Read upstream tests for fixture ideas, but do not count them as executed evidence. Each batch produces a permitted/forbidden observation table before a Rust implementation is claimed compatible.

| Batch | Concrete experiment | Required artifact and gate |
|---|---|---|
| A — Reference identity and local oracles | Build or obtain the pinned OTP version; execute synthetic demonitor flush, ordinary and priority mixed EXIT/DOWN ordering, catchable absent-target linking and receive timeout validation. | Executable/source identity plus allowed-result fixtures for REF-01/02, LANG-04 and G0. A locally installed OTP major version alone is insufficient. |
| B — Useful Rust admission | Compile a room/session body with owned payload operations through an external consumer crate; challenge transitive helpers, callbacks, destructors, partial state and nonreturning loops. | Positive/rejection fixtures and ownership argument for LANG-01–04, MEM-01 and G1. Keep automatic scheduling and safe kill as requirements. |
| C — Single-worker execution and death | Inject stop at each continuation/queue/identity transition; send each admitted control kind to running, exiting and absent processes, with requester death as an independent axis. | Exclusive-execution and release traces for SIG-01, LIFE-05/07, EXEC-01/03 and G2. No application execution after committed death; every late request has an explicit disposition. |
| D — Mailbox publication and wait | Place publication, buffer installation/removal, scan completion, expiry, cancellation and kill around controlled pause points; compare ordinary receive against later optimized receive. | Bounded transition model, counterexamples and differential traces for MSG-01–04 and G3; markers/priority remain conditional. Stress supplements the model. |
| E — Relationships and inspection | Exercise spawn publication, initial links/monitors, unlink/relink, demonitor and ordered liveness while processes stop; abandon inspection responses and enumerate during churn. | Ownership/order oracles for LIFE-01–07, OBS-01 and G4, restricted to supported variants. Remote loss-of-contact is a later contract. |
| F — Conditional timer/memory boundaries | Isolate pause/resume original-timer-reference continuity, creator versus recipient death, name rebinding, hibernate argument counts 255/256/257, full-sweep field boundaries and hibernate/max-heap interactions. | Versioned observations for TIME-01 and MEM-01. Keep source predictions unresolved until run; investigate reference behavior without adopting BEAM heap representation or APIs. |
| G — Later resources and workers | Model readiness cancellation versus safe close, recipient death, transfer/heir identity, interrupted cleanup and queue claim/wakeup across workers. | Separate I/O ownership and concurrency evidence for IO-01, RES-01 and EXEC-07/08. Shared tables and native external jobs need an explicit admitted-state contract before implementation. |

The first implementation sequence remains G0/G1 → single-worker execution/kill → ordinary messaging → relationships → raw room/session traffic → measurements. Independent reference investigations can proceed earlier; advanced APIs do not become prerequisites merely because their source is now documented.

## EXEC-08 — Advanced scheduling and explicit suspension

**Applicability:** Later priority/suspension APIs and G7 concurrency; exclusive claims and wakeup correctness already apply at G2.

**Build work:** Model user/effective/queued priority separately; represent queue records independently of execution ownership. Add deterministic claim, wake/park and transfer hooks. For explicit suspension, model per-suspender counts and acknowledgments; do not reuse ordinary scheduling preemption as this API.

**Dependencies:** EXEC-01/02/07, LIFE-05, TIME-01. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** Qualify exclusive execution and retained-record lifetime first. Extension acceptance requires counted suspension/death races, priority policy and timer behavior with no global fairness or deadline claim.

**Source and limits:** [Report 18](../research/18-process-execution-memory-deep-dive.md).

| Case | Branch and required qualification |
|---|---|
| EXEC-08.C01 | Low entry has normal peers, then becomes the sole entry **Qualification:** Rotation policy reduces its frequency with peers but does not repeatedly skip the only runnable entry; no hard ratio/deadline promise. |
| EXEC-08.C02 | Change user priority with active/delayed system tasks **Qualification:** Distinguish user/effective/queued priority; preserve pending higher-priority runtime work. |
| EXEC-08.C03 | Priority elevation while a lower-priority entry exists **Qualification:** Proxy and old records must still yield one real application execution owner; stale record disposal releases lifetime references. |
| EXEC-08.C04 | Process becomes inactive-free before a queue record is consumed **Qualification:** Discard the record safely without executing or retargeting it. |
| EXEC-08.C05 | Suspended process receives eligible system work or exit **Qualification:** Runtime work may proceed; application execution stays suspended unless lifetime termination supersedes it. |
| EXEC-08.C06 | Multiple suspenders and repeated owned suspend counts **Qualification:** Only final applicable resume releases suspension; suspender death removes its counts without cancelling other owners' suspensions. |
| EXEC-08.C07 | Async suspend races owned resume or target death **Qualification:** Consumer replies distinguish suspended/not_suspended/exited; enqueue failure has the separate tagged `badarg` branch in C18. Sending the request alone proves no immediate suspension. |
| EXEC-08.C08 | Self/remote/dead target, malformed options, absent resume ownership, count overflow **Qualification:** Preserve documented error distinctions; overflow must not leak an increment. Dangerous count/deadlock tests require isolated fixtures. |
| EXEC-08.C09 | Wake arrives before/after sleep-type publication or an OS wait is interrupted **Qualification:** No lost wakeup; correct poll/event poke; interruption alone must not lose timer or runnable work. |
| EXEC-08.C10 | Steal encounters bound tasks, contended victims or concurrent destination enqueue **Qualification:** Bound assignment remains protected; transferred list/counts stay consistent and no process executes twice. |
| EXEC-08.C11 | Online/active scheduler configuration changes while requesters exit **Qualification:** Complete or release ownership consistently; distinguish normal/dirty counts and partial asynchrony. |
| EXEC-08.C12 | Suspended receiver has receive timeout, PID timer and name timer **Qualification:** Apply OTP-versioned pause distinction and nested resume ownership; timer reconstruction details require the timer audit's cases. |
| EXEC-08.C13 | Same requester repeats a request; second requester also suspends. Exhausting the first owner's repeats leaves the second effective. **Qualification:** Reject target/timer decrements per repeat instead of per activated relationship; use the ownership test cited in report 26. |
| EXEC-08.C14 | Dirty-running target receives suspend then final resume before activation. Removed target record is skipped at the dirty boundary; retained reference is released. **Qualification:** Forbid phantom suspension, negative count and use-after-free. Need controlled interleaving fixture. |
| EXEC-08.C15 | Requester dies with a pending dirty suspension and pending tagged reply. Demonitor cancels its relationship; reply to missing requester is reclaimed. **Qualification:** Forbid leaked relationship or stranded target; existing regression covers part, not every ack race. |
| EXEC-08.C16 | Target begins exit with pending suspension. Drain must skip activation; queued acknowledgment uses exit state when serviced. **Qualification:** Forbid resurrection or success acknowledgment based solely on old `ACTIVE`; full native-exit/ack timing remains unexecuted. |
| EXEC-08.C17 | Immediate versus queued activation with a receive timeout and PID timers; add another owner and release owners separately. **Qualification:** Forbid partial release restarting still-owned timers or application execution through the transition; timer-expiry crossing needs an oracle. |
| EXEC-08.C18 | Tagged async request to known-dead target, contrasted with ordinary sync request. **Qualification:** Report 29 observes tagged `badarg` and early synchronous `badarg` exception. Do not substitute documented `exited` or claim sync early rejection traverses the send helper; the compatibility discrepancy remains explicit. |
| EXEC-08.C19 | Synchronous request passes initial relationship/send stage, then target disappears before ack enqueue. **Qualification:** Keep helper fallback and exiting-consumer results distinct; reproduce the narrow race before selecting compatibility. |
| EXEC-08.C20 | Multiple tagged requests accumulate during dirty work; requester may release its relation while another owner remains. **Qualification:** Do not infer FIFO acknowledgments or per-owner activity from a target-state reply. Characterize tags/state without inventing an ordering guarantee. |
| EXEC-08.C21 | Kill and final resume race with internal suspension monitor-down, including origin already removed. **Qualification:** Forbid duplicate release, application-visible internal `DOWN` and stale origin; `erl_704` is the direct regression oracle. |
| EXEC-08.C22 | Target is suspended while GC/control work is requested; contrast dirty application work with dirty system work holding `MAIN`. **Qualification:** Keep application continuation suspended during GC; do not assume universal prompt control service. Measure eligible work separately from latency. |

**Deeper evidence:** [Report 26](../research/26-suspension-system-work-protocol.md) extends this group with protocol invariants and remaining limits. [Pinned reference observations](../research/29-pinned-reference-observations.md) cover selected variants, including discrepancies; the feature is not qualified for Bream.

**System-task, code-lifetime and routing refinements:** [report 50](../research/50-system-task-dispatch-and-dirty-handoff.md). These are planned cases; source-only, executed reference and Rust compiler evidence remain separate. Optional naming, native jobs and code replacement do not become initial requirements.

| Case | Branch, required treatment and acceptance evidence |
|---|---|
| EXEC-08.C23 | Pause task admission around its state transition and list insertion; make its operation-specific failure condition win on another attempt. Track exactly one owner for accepted, rejected and redirected requests. A rejected task left linked, early consumer access before linking, duplicate reply/free, or losing a task when a competing publisher already activated the target. Use the failed-enqueue regression as an oracle seed. |
| EXEC-08.C24 | Request GC/code inspection with and without pending signals, including inherited priority and a target changing between normal and dirty application execution. Trace direct admission, ordered RPC and dirty-handler redirection. Bypassing the required signal order, using effective priority as the inherited user priority, or both the RPC and the task producing the one correlated result. Conditional operation-specific reference mapping. |
| EXEC-08.C25 | Keep active max/high user work while queuing lower-priority tasks, then make the process inactive/suspended; separately drain normal/low task classes with each queue emptying first. Treating every task as higher priority than user work, confusing task interleaving with process run-queue policy, or turning a fixture's ratio into a deadline promise. |
| EXEC-08.C26 | Delay GC-dependent tasks, enqueue newer tasks in the same classes, then re-enable GC or begin exit. Preserve delayed request ownership, priority contribution and front insertion without silently nesting disable/enable ownership. Orphaned delayed work, reply before prerequisite work is permitted, tail insertion claimed as the reference order, or a task present in both queues. |
| EXEC-08.C27 | Queue repeated minor/major work and dirty-deferred requests with distinct correlation values; make one operation satisfy several requests and let some requesters disappear before settlement. Losing individual completion obligations when coalescing work, duplicate computation/replies, leaked task payloads, or inventing FIFO across the dirty pending list. |
| EXEC-08.C28 | Build a large completion list after one dirty system operation, including expensive correlation payloads and requester contention. For a proposed bounded Rust completion path, pause between replies and inject exit/shutdown. Dropping unprocessed requests, replaying finished replies, acknowledging complete cleanup early, or claiming the operation's offload/budget bounds reply fanout. This is a planned cost experiment, not an observed ERTS defect. |
| EXEC-08.C29 | A signal-priority service task exhausts its allowance, reschedules, receives more signals, then races exit or rescheduling rejection. Re-fetching as though every attempt were the first, losing the retained task, executing application code after exit, or dropping the exit-service elevation prematurely. Distinct from public priority mailbox insertion. |

## MEM-01 — Owned memory, GC reference branches and hibernation

**Applicability:** Initial ownership/accounting at G1–G2; GC knobs, allocation-mode controls and hibernation are representation research or later capabilities.

**Build work:** Inventory every live root/owner in admitted Rust state, messages, partial helpers and runtime metadata. Specify release and deferred work. Use BEAM collection branches as lifetime counterexamples; design a Rust ownership model instead of copying its moving heap. Only expose memory policies or hibernation after their native contract is justified.

**Dependencies:** LANG-01/02/03, EXEC-02/03/04, LIFE-05, MSG-01/04. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** Provide an ownership table for every suspension/stop point, fault coverage and final-owner accounting. Qualify any chosen compaction or layout change separately; no GC API parity or process quota is selected.

**Source and limits:** [Report 18](../research/18-process-execution-memory-deep-dive.md).

| Case | Branch and required qualification |
|---|---|
| MEM-01.C01 | Collection with roots in registers, stack, dictionary, saved arguments, failure state and receive markers **Qualification:** Every live reference remains valid; partially initialized signal slots are not scanned as terms. Rust needs its own admitted-state proof. |
| MEM-01.C02 | Minor collection fits versus old-generation/virtual-binary space exhaustion **Qualification:** Correct minor result or major fallback, including dirty reclassification; no double collection accounting or lost live state. |
| MEM-01.C03 | `fullsweep_after` zero, normal positive and values around stored-field width **Qualification:** Report 29 observes 65536→0 and 65537→1 round-trips. Actual major/minor decisions and other option interactions still need qualification; no Rust memory policy is selected. |
| MEM-01.C04 | GC disabled/delayed, with zero and positive allocation need **Qualification:** Preserve abandoned heaps and stack roots; represent allocation work and eventual collection rather than losing fragments. |
| MEM-01.C05 | Async GC queued during disabled GC, then enable or death **Qualification:** No premature success; delayed requests become serviceable on enable, while death ordering uses the established signal contract. |
| MEM-01.C06 | Maximum heap disabled, equality boundary, exceedance, kill/log combinations and shared-binary option **Qualification:** Compare the correct accounting domain; distinguish diagnostics from termination and candidate collection space from live bytes. |
| MEM-01.C07 | Minimum heap/virtual-binary settings change **Qualification:** Verify validation/rounding and later allocation effect; do not infer immediate resize or an exact process-memory floor. |
| MEM-01.C08 | Message fragment is consumed, retained by process state, then collected **Qualification:** Transfer off-heap ownership once; freeing the queue node must not invalidate retained payload. |
| MEM-01.C09 | On→off queue conversion with arrivals and GC **Qualification:** Transitional roots remain covered until prior sender activity/conversion completes; mode setter return is not physical conversion completion. |
| MEM-01.C10 | Repeated allocation-mode reversal, conversion yield, then process death **Qualification:** Follow last requested state, retain continuation markers safely and free conversion state on exit. |
| MEM-01.C11 | Dead young versus old-generation binary reference, then final shared owner release **Qualification:** Minor/full-sweep differences do not become leaks; never free another holder's bytes; final physical release happens once. |
| MEM-01.C12 | Magic/native resource reaches zero with outstanding resource-monitor references **Qualification:** Deferred destructor completion remains accounted; callback duration is not automatically bounded and no premature physical free occurs. |
| MEM-01.C13 | `hibernate/3` bad arguments, 255/256/257 argument lists, improper tails or missing resume MFA **Qualification:** Report 29 observes post-stack-reset `undef` for the 255/256 length fixture and catchable pre-reset `system_limit` at 257. Improper tails and other targets remain untested. Accepted hibernation discards old catches/stack and later applies the target. |
| MEM-01.C14 | `hibernate/0` with deep stack/live locals versus `/3` **Qualification:** Preserve `/0` return continuation and live state; do not conflate with `/3` stack reset. |
| MEM-01.C15 | Existing message or arrival between pre/post-compaction fetches, including dirty hibernation **Qualification:** No stranded runnable process; memory optimization must not erase wakeup ownership. |
| MEM-01.C16 | Hibernation below minimum, later GC and memory-limit settings **Qualification:** Verify minimum restoration and peak/limit interactions experimentally; hibernate compaction is a separate path, not a blanket resource-cap promise. |

**Rust ownership/source refinements:** [report 38](../research/38-process-heap-roots-and-rust-ownership.md). These cases are planned qualification obligations. BEAM representation-specific branches are conditional reference work; a Rust alternative must meet the supported observable and ownership contract. Language-probe results do not qualify Bream features.

| Case | Branch, required treatment and acceptance evidence |
|---|---|
| MEM-01.C17 | Post-BIF success, trap and exception use different explicit live arrays; successful return can become nonvalue on EXITING (§1). Exercise generated success/error/suspend transitions with independently dying arguments and live results; no lost result, stale register read, or resumption after terminal transition. |
| MEM-01.C18 | Known-live fragment prefix contains dead temporaries; first disabled-GC trap saves the marker, later traps preserve it; prior GC/abandoned heap invalidates it (§3). Preserve the selected retention boundary across repeated helpers; never walk a freed marker or promise immediate garbage removal. Account retained bytes and reclaim all admitted allocations on death. |
| MEM-01.C19 | Duplicate roots and mature descendants cross a minor collection, followed by dropping old roots and a full sweep (§2). Preserve values and required ownership sharing through promotion closure; no surviving old reference into evacuated young storage. A mutable Rust graph must use an explicitly correct policy rather than assume BEAM's immutable invariant. |
| MEM-01.C20 | Heap/stack grow or shrink, with same-base and moved-base realloc outcomes (§4). Preserve continuation and every external owner/reference; test independent stack displacement. A Rust representation may avoid relocation, but may not byte-move borrowed or pinned state without satisfying its contract. |
| MEM-01.C21 | A binary match context retains on-heap bytes through owner relocation; compare BinRef-backed views (§4). Preserve owner, bit offset/length and subsequent reads. Repair or avoid cached interior pointers; distinguish descriptor movement from backing-byte movement. |
| MEM-01.C22 | Writable BinRef is old-generation, young-forwarded or dead-young; frozen backing moves to the regular list, with active/inactive shrink candidates (§5). Each backing remains in exactly one applicable ownership structure, accounting matches its state, and final release occurs once. Preserve all published bytes and measure spare capacity rather than treating shrink as mandatory semantics. |

## STATE-01 — Process-local dictionary and metadata

**Applicability:** Owned-state invariants apply initially; a dynamic dictionary API remains an extension.

**Build work:** Keep process-local state and metadata in the ownership inventory. If a dictionary is proposed, define typed key/value admission, missing/old-value results, snapshot ownership and resumable enumeration before implementing it.

**Dependencies:** LANG-02, EXEC-02/03, MEM-01, OBS-01. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** Snapshots remain independent; replacement/erase results survive cleanup transitions; no dictionary path admits arbitrary callbacks or shared mutable application state.

**Source and limits:** [Report 18](../research/18-process-execution-memory-deep-dive.md).

| Case | Branch and required qualification |
|---|---|
| STATE-01.C01 | First put, replacement and missing get/erase **Qualification:** Preserve creation, old-value and absent-value semantics; a typed Rust representation is still unselected. |
| STATE-01.C02 | Immediate replacement after `get/0` or dictionary inspection **Qualification:** Earlier snapshot values remain unchanged despite internal in-place optimization. |
| STATE-01.C03 | Replacement triggers GC while the old return value is otherwise unreachable **Qualification:** Key/new value/returned old value all survive; result ownership cannot be dropped with the old bucket. |
| STATE-01.C04 | Erase triggers bucket shrink and GC **Qualification:** Preserve the erased return value and remaining entries; no premature root-count reduction. |
| STATE-01.C05 | Dictionary is the remaining owner during hibernation or process death **Qualification:** Live entries survive compaction; death releases owned references with the process, respecting external shared owners. |
| STATE-01.C06 | Large dictionary enumeration/grow/shrink or metadata expansion **Qualification:** Qualify work and allocation cost transitively; no hidden uninterruptible callback or false constant-time claim. |

## MSG-05 — Aliases and reply-address lifetime

**Applicability:** Later typed addressing capability; ordinary PID send remains the first profile.

**Build work:** Model inactive, ordinary, one-shot and monitor-associated alias states. Separate receiver acceptance from application selection, and couple alias/ref lifetime to the owning process without making an address an owner of execution.

**Dependencies:** MSG-01/04, LIFE-02/05/07, LIFE-01. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** Fixtures place deactivation and death on both sides of delivery conversion; each payload is retained or released once, and accepted older messages are not implicitly flushed.

**Source and limits:** [Report 19](../research/19-process-signals-timers-deep-dive.md).

| Case | Branch and required qualification |
|---|---|
| MSG-05.C01 | **Alias delivery cut** — Deactivate before send, after publication/before handling, and after mailbox conversion. Drop only not-yet-received alias deliveries; never flush an older received item implicitly. |
| MSG-05.C02 | **One-shot alias** — Two replies plus unrelated unmatched backlog. First accepted reply deactivates alias independent of application selection; later alias reply drops. Test competing senders as a permitted set. |
| MSG-05.C03 | **Alias state/priority** — Never-alias reference, pending spawn alias, ordinary alias, priority alias × ordinary/priority send. No active-state side channel through ordinary successful send. |
| MSG-05.C04 | **Alias ownership/kill** — Stop recipient at copy, publication, drop and conversion; exactly one owner/release for payload and immutable-buffer refs. Share relationship cleanup fixture with LIFE. |

## MSG-06 — Priority message reception

**Applicability:** Later mailbox feature, separate from scheduler priorities.

**Build work:** Represent capability gating and ordinary/priority mailbox order separately from signal arrival order. Specify receive cursor redirection and last-capability removal before choosing a queue layout. Account reference searches and prefix cleanup.

**Dependencies:** MSG-02/04/05, LIFE-02/03/07, EXEC-02. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** Compare allowed signal traces and selectable mailbox order; check all cursor positions and lifecycle notifications. Ordinary-only contracts must remain unchanged when priority reception is disabled.

**Source and limits:** [Report 19](../research/19-process-signals-timers-deep-dive.md).

| Case | Branch and required qualification |
|---|---|
| MSG-06.C01 | **Priority order** — Mixed application/Exit/Down prefix order preserves each class; same-sender priority can overtake ordinary content, without reordering internal signals. |
| MSG-06.C02 | **Priority during receive** — Insert while cursor is ordinary, priority, exhausted or waiting. Newly eligible priority message must be seen; continuation must resume without losing ordinary content. |
| MSG-06.C03 | **Last priority capability** — Remove last alias/link/monitor with prefix nonempty; then re-enable. Already accepted messages remain, and one-shot first reply retains priority. |
| MSG-06.C04 | **Reference search cost** — Large nested priority payload containing/no matching marker reference. Verify both eligible-message equivalence and scheduler/kill responsiveness; no unbudgeted walker. |
| MSG-06.C05 | `RCVM` × relevant/unrelated priority payload; reference nested in tuple, list, map and closure environment. **Qualification:** Optimized/unoptimized selection and remainder agree. Forbid missing a relevant priority reply; measure unnecessary scanning separately. |
| MSG-06.C06 | `FIRST→MARK→FIRST` or `RCVM→MARK→RCVM`; multiple priority arrivals during scanning and waiting. **Qualification:** No duplicate/lost selections or skipped ordinary suffix. Instrument cursor steps to check continuation restoration rather than relying on elapsed time. |
| MSG-06.C07 | Last capability removed with unmatched prefix and active receive reference; re-enable before versus after drain. **Qualification:** Existing priority messages must remain ordered and selectable. Report 29 reproduces the optimized debug-interpreter assertion with passing unoptimized/release-JIT controls; remaining re-enable and race products stay open. |
| MSG-06.C08 | Internal `LAST` wait crossed by priority messages and its ordinary private reply. **Qualification:** Internal operation completes; priority messages remain for the next user receive. Forbid exposing `LAST` as an arbitrary application shortcut. |
| MSG-06.C09 | Reference observed in one priority epoch, its message consumed, prefix later drained/recreated. **Qualification:** Conservative stale evidence may cost scans but cannot lose messages. Check bit reset/new epoch through instrumented state, not public mailbox length alone. |

**Deeper evidence:** [Report 28](../research/28-selective-receive-marker-protocol.md) extends this group with protocol invariants and remaining limits. [Pinned reference observations](../research/29-pinned-reference-observations.md) cover selected variants, including discrepancies; the feature is not qualified for Bream.

## MSG-07 — Receive-marker optimization

**Applicability:** Optional optimization after correct unoptimized receive.

**Build work:** Define a provable freshness boundary, marker ownership, reserve/bind/use/clear transitions and fallback under pool pressure. Preserve marker state through queued signals, error, timeout and death.

**Dependencies:** MSG-02/03/06, LANG-03, LIFE-05. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** Differentially compare optimized and unoptimized receive traces under the same partial orders. A missing or retired marker may cost extra work but must not lose an eligible message.

**Source and limits:** [Report 19](../research/19-process-signals-timers-deep-dive.md).

| Case | Branch and required qualification |
|---|---|
| MSG-07.C01 | **Marker reserve/bind/clear** — Empty/nonempty queues, valid/invalid bind, absent/retired marker, pool pressure and big temporary IDs. Optimization may change inspections, never selected messages. |
| MSG-07.C02 | **Deferred marker + priority** — Marker in middle, fresh reply in priority prefix, nested use and timeout/error reset. Compare optimized/unoptimized legal traces. |
| MSG-07.C03 | Fresh-reference receive versus guard alternative not requiring it; immediate monitor reply; helper also called with non-reference tag. **Qualification:** Confirm instruction presence/absence and equal behavior. Forbid skipping an old message accepted by any unqualified clause. |
| MSG-07.C04 | Marker in middle; use superseded by another use, normal receive or timeout; later marker handling. **Qualification:** At most one pending cursor writer. Forbid a stale marker overwriting a subsequent receive's save position or appearing as a message. |
| MSG-07.C05 | More than eight nested candidates, explicit clears, caught exceptions, GC/relocation and full middle-marker pool. **Qualification:** Fall back without semantic change; bound metadata, retain live terms safely, and reclaim orphan slots. Performance loss is distinct from correctness failure. |
| MSG-07.C06 | No-match advance followed by match, finite/zero timeout or invalid timeout; then another receive. **Qualification:** Retained messages remain selectable from the proper start; invalid timeout cannot leave save on metadata. Do not assert a universal message/expiry race winner. |
| MSG-07.C07 | Kill with inner/middle markers, pending save, priority end and continuation. **Qualification:** Application execution never resumes; runtime-owned marker storage is eventually reclaimed once. Assert stopped execution separately from memory return. |

**Deeper evidence:** [Report 28](../research/28-selective-receive-marker-protocol.md) extends this group with protocol invariants and remaining limits. [Pinned reference observations](../research/29-pinned-reference-observations.md) cover selected variants, including discrepancies; the feature is not qualified for Bream.

## TIME-01 — General message timers and paused-timer identity

**Applicability:** Later timer API; receive timeout refinements remain in MSG-03. Explicit suspension is separately conditional.

**Build work:** Separate creator, recipient and service ownership. Define state transitions for expiry, synchronous/asynchronous cancel/read, recipient death and named lookup at expiry. Preserve wrapper/reference ownership. Reconcile report 29's observed paused-reference discontinuity with the intended compatibility contract before settling the Rust result.

**Dependencies:** MSG-01/03/04, LIFE-01/05, EXEC-02/03/07/08, NAME-01. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** Check response shapes and permitted race outcomes, independently of mailbox contents. Record logical cancellation, payload release and service cleanup separately; ordinary preemption must not pause deadlines.

**Source and limits:** [Report 19](../research/19-process-signals-timers-deep-dive.md).

| Case | Branch and required qualification |
|---|---|
| TIME-01.C01 | **Timer input/payload** — `start_timer` versus `send_after`, zero/large/absolute-past/range-overflow, bad options and alias/nonlocal destinations. Correct result/error and wrapper; no receive-range reuse. |
| TIME-01.C02 | **Creator/recipient/name** — Creator dies, PID recipient dies, name unregisters/rebinds before expiry. Preserve recipient cancellation versus late name lookup; no creator-owned automatic cancellation. |
| TIME-01.C03 | **Cancel/read mode matrix** — Current/other scheduler × caller-is-recipient/other × sync/async × info. Check response shape, suppressed notifications and caught errors; no false-as-mailbox-empty claim. |
| TIME-01.C04 | **Expiry/cancel race** — Enumerate ACTIVE CAS outcomes; permit both timeout/cancel-false arrival orders. Integer cancel cannot also produce that active timer's timeout; no implicit mailbox retraction. |
| TIME-01.C05 | **Timer cleanup progression** — Many same-deadline timers and recipient-death cancellation across services. Account for payload release and peer progress separately from logical cancellation. |
| TIME-01.C06 | **Suspended timers** — Receive/PID BIF/name BIF × nested pause/new timer/death. Reference fixtures establish remaining-time behavior; normal scheduler preemption must not pause deadlines. |
| TIME-01.C07 | **Paused reference continuity** — Report 29 observes failed original-ref lookup/cancel with later payload delivery, including separate read-only controls and timers created during suspension. Reconcile this observed discontinuity with the intended contract; expiry/resume/death race products remain open. |

**Executed reference subset:** [Report 29](../research/29-pinned-reference-observations.md) covers the paused-reference variant. Its creator-death fixture does not establish that expiry occurred after death; the stronger lifetime rule remains source-supported.

**Performance-source refinements:** [report 36](../research/36-timer-service-costs-and-batching.md). All remain planned. Reference-specific representations and optional/later capabilities are conditional; the Rust implementation must preserve the applicable ownership and observable behavior, not copy the storage layout.

| Case | Branch, required treatment and acceptance evidence |
|---|---|
| TIME-01.C08 | Cross both allocation-preference and wheel/HL routing thresholds at −1/exact/+1; repeat relative/absolute forms and selected wheel configurations. Record actual backend and pool fallback. Treating 5 seconds as the tree boundary; changing accepted deadlines or rounding while switching representation. Sources: [parse](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2494-L2641), [allocation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L875-L974). |
| TIME-01.C09 | Hit at-once, last soon position, exact soon boundary and later wrap; delay service so a later timer is retained, promoted or already overdue. Early expiration, duplicate/missing callback, or infinite revisit of a future collision. [Wheel set](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L1256-L1309); [later](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L1030-L1167). |
| TIME-01.C10 | Exhaust wheel work with a nonempty detached sentinel, then cancel its first/middle/last remaining member before resume. Stale slot pointer, double unlink/free, canceled entry executing, or loss of other entries. [Removal](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L688-L749); [soon](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L913-L1026). |
| TIME-01.C11 | Cancel HL list member, representative with survivors, singleton minimum, and representative after expiry traversal detached the group. Lost replacement minimum, duplicate service timer, resurrection, or premature release of traversal-owned state. [HL deletion](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1451-L1510); [service](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1395-L1447). |
| TIME-01.C12 | Large exact-deadline HL group with one runnable peer; trace repeated at-once rearm at/after budget exhaustion. Compare singleton and multi-entry at-once lists. Declaring a scheduler bound from the inner limit alone. Actual return/dispatch trace remains an exact-version execution question. [Critical branch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L842-L894); [service](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1395-L1447). |
| TIME-01.C13 | Sparse later-only wheel, delete cached minimum, insert earlier deadline, and exercise a maintenance/pre-poll wakeup with a distant surviving timer. Sleeping past a newly earlier deadline, early delivery at maintenance wakeup, or confusing the week poll cap with a user-timeout limit. [Search](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L476-L644); [set](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L1256-L1309). |
| TIME-01.C14 | Contend remote-cancel producers on tail and alternate insertion; stop after a small final batch near a thread-progress boundary. Trace publish, eligibility, notify, drain and empty-state transitions. Stranded eligible work without continuation, freeing a producer-accessible node, or lost/double owner cleanup. No global concurrent FIFO requirement. [Queue](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1561-L1609); [progress](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1634-L1686); [aux](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L2187-L2272). |
| TIME-01.C15 | Equal timer counts with immediate, copied compound and off-heap-resource payloads; expire/cancel/kill recipient on local and remote-owner paths. Record time inside each nominally charged callback. Equating 80 reductions/100 entries with a time cap; leaking/double-releasing payload on the winning transition. [Payload](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L722-L755); [exit](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2402-L2483). |
| TIME-01.C16 | Separate successful logical cancel, owner index removal, final process-record release and timer allocation reuse; introduce a delayed progress participant and allocation pressure. Reuse before the timer's retirement protocol permits it; treating a response, empty recipient tree or retained allocator arena as proof of completed reclamation. [Owner cleanup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1519-L1542); [final wheel release](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L802-L835); [final HL release](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1080-L1120). |

**Concurrency/clock refinements:** [report 48](../research/48-clock-domains-and-process-deadlines.md). All cases remain planned. Source-specific coordinator/lock/provider details are conditional reference work, not a requirement to reproduce ERTS representation. Small model completion does not qualify Bream.

| Case | Branch, required treatment and qualification |
|---|---|
| TIME-01.C17 | With corrected multi-warp, hold finite receive and BIF deadlines while injecting forward/backward wall-clock steps. Observe monotonic deadline, system offset and later notification separately. **Qualification:** Retiming an elapsed deadline from wall offset; requiring notification delivery before expiry; assuming atomic equality across separately sampled domains. [Offset](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_time_sup.c#L779-L831), [paired reads](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/unix/sys_time.c#L506-L535). |
| TIME-01.C18 | Exercise correction/provider dispatch and no/single/multi-warp compatibility branches. Race two single-warp finalizers; verify one preliminary→final transition and unchanged timer coordinate. **Qualification:** Claiming correction-disabled mode still reads OS monotonic time; assuming every mode has identical freeze/rate behavior. Bream compatibility-mode admission remains unselected. [Dispatch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_time_sup.c#L1234-L1287), [fallback](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_time_sup.c#L951-L984), [finalize](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_time_sup.c#L1330-L1414). |
| TIME-01.C19 | Distinguish configured suspend-excluding/inclusive clocks, repeated readings, historical regression verification and provider read failure; separately record runtime scheduling/preemption. **Qualification:** Using CPU time, treating repeated readings as duplicate timer identity, pausing deadlines on ordinary preemption, or asserting one cross-platform suspend contract. Failure injection must use the documented supported provider contract. [Configuration](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/make/autoconf/otp.m4#L2524-L2547), [providers](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/unix/sys_time.c#L469-L591). |
| TIME-01.C20 | Place setup/read/cancel just before, exactly on and after a tick boundary; include positive relative, absolute, BIF zero and receive zero. Delay service while the timer remains active. **Qualification:** Early firing through floor rounding; treating integer remaining-time zero as false, expired or delivered; allocating a receive-zero timer merely to share a helper. Fractional active-zero outcome still needs execution. [Parse](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2493-L2559), [remaining](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1921-L1948), [receive](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2759-L2828). |
| TIME-01.C21 | Cross negative external origin, signed unit conversion, runtime start/end translation and final representability limits; use large intermediate products whose quotient fits and ones whose result does not. **Qualification:** Truncation-toward-zero replacing mathematical floor; rejecting all negative absolute timestamps; wraparound into an accepted near deadline. General parser boundary tests already in C01 remain prerequisites. [Origin](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_time.h#L312-L338), [conversion](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L5317-L5339), [range](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2493-L2559). |
| TIME-01.C22 | Cause repeated empty poll returns before the deadline, waits exceeding `INT_MAX` milliseconds, and a provider rate differing from the kernel wait clock. Trace every derived remaining interval and final Erlang-time check. **Qualification:** Resetting the full original duration, accepting a capped/early kernel return as user expiration, or spinning indefinitely once genuinely due. Events/errors still follow their separate poll protocol. [Conversion](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/common/erl_poll.c#L1697-L1743), [recheck](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/common/erl_poll.c#L1949-L1968). |
| TIME-01.C23 | Register near-deadline timers after lengthy work with an old cached clock; transfer already-constructed deadlines between workers; delay expiry batches and inspect the next fresh sample. **Qualification:** Constructing a relative deadline from a sample predating the allowed registration observation; mixing runtime origins; restarting elapsed duration during transfer. A stale servicing sample may add lateness, never license early expiry. [Read](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_time_sup.c#L1889-L1904), [scheduler](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L9797-L9809). |
| TIME-01.C24 | Force reads around previous/current correction-instance cutover and concurrently publish an offset update; cover equal readings and explicit backward-provider faults. **Qualification:** Torn correction fields, backwards corrected time at a valid transition, treating the future instance as applicable too soon, or changing stored timer positions on a system-offset notification. Full numerical/weak-memory qualification remains required. [Reader](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_time_sup.c#L345-L400), [writer](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_time_sup.c#L736-L770), [atomic offset](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_time_sup.c#L202-L219). |

## LIFE-06 — Expanded spawn options and request lifetime

**Applicability:** Initial atomic-spawn refinements plus later asynchronous/remote request APIs. Each conditional case applies only when that capability is admitted.

**Build work:** Record validation before versus after request creation, option parsing order, reply ownership and initial relationship publication. Model pending, replied, abandoned and parent-dead states. Remote variants need connection incarnation and a distinct loss-of-contact contract.

**Dependencies:** LIFE-01/02/03/05, MSG-04, EXEC-03/04, REF-01/02. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** Inject failures/death before and after publication. Check child existence separately from request/reply state; abandonment cannot be treated as rollback. Do not implement transport from these boundary notes alone.

**Source and limits:** [Report 20](../research/20-process-lifecycle-deep-dive.md).

| Case | Branch and required qualification |
|---|---|
| LIFE-06.C01 | Wrong argument kind or improper args/options list → synchronous argument error. **Qualification:** Enumerate overload/type validation apart from async errors; map Rust compile-time exclusions explicitly. |
| LIFE-06.C02 | Proper list with invalid option → request ref plus enabled error/badopt reply; spawn_opt raises. **Qualification:** Keep front-door versus operation failure distinct. |
| LIFE-06.C03 | Invalid option plus reply/tag options in both orders → local/remote precedence oracle. **Qualification:** Do not normalize away parser-order effects. |
| LIFE-06.C04 | Each reply policy × success/error → exactly enabled reply class. **Qualification:** yes/no/success-only/error-only; suppressing success still creates child. |
| LIFE-06.C05 | Compound reply tag × success/failure/abandon/parent death → valid payload or one safe release. **Qualification:** Pending tag ownership; typed tag API unresolved. |
| LIFE-06.C06 | Atomic link+monitor request, immediately exiting/sending child → enabled success reply delivered before child signals. **Qualification:** With default reception priority, collect complete mailbox sequence; separately test signal delivery versus priority placement. |
| LIFE-06.C07 | Local completed request, reply unread → abandon false. **Qualification:** Unread reply is not pending operation. |
| LIFE-06.C08 | Remote completion races abandon → one effective terminal state; nonref errors, unrelated/finished refs false. **Qualification:** Gates for pending/received/abandoned states and owner-local lookup. |
| LIFE-06.C09 | Abandon linked request wins, then child created on still-usable original connection → child gets abandoned, parent no operation reply/relationship. **Qualification:** Child may trap reason or send other messages; no rollback claim. |
| LIFE-06.C10 | Parent dies before remote linked request completes → child gets parent reason or prior abandoned reason. **Qualification:** Retain pending operation and original connection in cleanup continuation. |
| LIFE-06.C11 | Success reply suppressed but link/monitor requested → relationships still activate on success. **Qualification:** Reply presence cannot be sole activation event. |
| LIFE-06.C12 | Created child has missing MFA/argument decode failure → success then child failure. **Qualification:** Rust entry admission differs; define applicable analogue, not fictitious launch failure. |
| LIFE-06.C13 | Disconnect during remote request → noconnection; child may exist. **Qualification:** Never infer not-created or remote-stopped universally. |
| LIFE-06.C14 | Stale reply/old connection → release attached ownership; no resurrection/new-connection effect. **Qualification:** Inject stale control representations in later internal tests. |
| LIFE-06.C15 | Remote synchronous wrapper failure → preserve documented PID/monitor/exception shape. **Qualification:** Later compatibility layer needs crasher fallback; not local-first prerequisite. |
| LIFE-06.C16 | Duplicate monitor/link option forms → versioned order oracle. **Qualification:** Plain monitor after tagged/aliased form; local versus distributed rewrite. |
| LIFE-06.C17 | Initial options/capacity failure → settings before first run; no live partial child/relation on failure. **Qualification:** Couple EXEC accounting; allocator fault-injection proof outstanding. |

## LIFE-07 — Relationship options, references and exit continuations

**Applicability:** Initial relationship/exit refinements plus conditional alias, tag, priority and remote variants.

**Build work:** Extend endpoint state tables with owned tags/options, reference provenance, alias survival and repeated link mutation. Preserve cleanup continuations and late replies after identity removal. Bind deferred remote work to its original connection.

**Dependencies:** LIFE-01/02/03/04/05, MSG-04/05/06, EXEC-03, SIG-01. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** Test endpoint ownership and cancellation separately from application consumption. Resolve synthetic-flush and combined-notification oracles before claiming exact parity; every exit phase must retain and release its owned work.

**Source and limits:** [Report 20](../research/20-process-lifecycle-deep-dive.md).

| Case | Branch and required qualification |
|---|---|
| LIFE-07.C01 | Immediate/compound custom DOWN tag, normal/missing target → valid payload through receipt/discard. **Qualification:** Ownership across allocation, kill, cancel and queued notification cleanup. |
| LIFE-07.C02 | explicit_unalias monitor cancelled → standalone alias survives, observation ends. **Qualification:** Replacement alias must not retain monitor behavior. |
| LIFE-07.C03 | explicit_unalias gets DOWN, including noproc → alias survives until unalias/owner death. **Qualification:** Preserve alias while destroying monitor/tag endpoints. |
| LIFE-07.C04 | demonitor/reply_demonitor modes × cancel/DOWN/first reply/unalias → mode contract. **Qualification:** Couple alias delivery; no duplicate release. |
| LIFE-07.C05 | Late compound DOWN after monitor-to-alias replacement → no notification resurrection. **Qualification:** GH5310 schedule, immediate and compound reasons. |
| LIFE-07.C06 | Repeated link priority toggle → caller endpoint only, one relation. **Qualification:** Distinguish scheduling/reception priorities. |
| LIFE-07.C07 | Last priority relationship exits/cancels/abandons → delete bookkeeping once. **Qualification:** Surviving priority alias separate from removed monitor. |
| LIFE-07.C08 | Self-monitor with alias/tag/priority → ref, no relationship. **Qualification:** Exact option-effect oracle; no phantom alias. |
| LIFE-07.C09 | Wrong-owner local/remote ref to demonitor → documented cheap checking. **Qualification:** Never cancel another owner's relationship; Rust error/provenance surface unresolved. |
| LIFE-07.C10 | Pending spawn ref or standalone alias to demonitor → no process-monitor removal. **Qualification:** Distinct reference kinds/states. |
| LIFE-07.C11 | Old creation versus remote disconnect → noproc versus noconnection. **Qualification:** Never infer remote stop from lost connection. |
| LIFE-07.C12 | Full ref-counter/runtime-instance reuse → explicit identity policy. **Qualification:** Tiny-counter model, serialization and retained-work tests; full wrap proof absent. |
| LIFE-07.C13 | Exit owning timer/block/ETS-equivalent → required pre-notification release, resumable phases. **Qualification:** Only admitted kinds apply; callbacks not presumed bounded. |
| LIFE-07.C14 | Compound reason across cleanup yields/GC-equivalent work → stable committed reason. **Qualification:** Runtime-owned reason outlives all notification continuations. |
| LIFE-07.C15 | Late monitor/link/unlink/info/RPC → correct response/release in exit state. **Qualification:** Expand LIFE-05.C10 dispatcher variants; no application resurrection. |
| LIFE-07.C16 | Kill during fragmented transport/pending linked spawn → finish runtime protocol obligations. **Qualification:** Later distribution cleanup; retained refs tracked after DOWN. |
| LIFE-07.C17 | Synthetic versus genuine tuples, active/absent flush/info → exact oracle and typed-envelope mapping. **Qualification:** Docs/source discrepancy open; do not silently choose constructors/matching. |
| LIFE-07.C18 | Combined EXIT/DOWN × local/remote, immediate/compound, mixed priority → reference order/allowed set. **Qualification:** Refine existing case with catch-all traces and sender/relationship metadata. |

## NAME-01 — Local name registration lifetime

**Applicability:** Later addressing/registry capability, independent of supervision.

**Build work:** Model the registry association as its own lifetime: validation, duplicate registration, lookup, unregister, rebinding and exit removal. Keep address resolution and delivery distinct; define any Rust name type without importing a global atom leak.

**Dependencies:** LIFE-01/02/05, MSG-01/04, EXEC-03/04. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** Exercise replacement races and name reuse around death/Down; cleanup must not remove a newer association. Named monitoring binds the observed lifetime and preserves its reported object.

**Source and limits:** [Report 20](../research/20-process-lifecycle-deep-dive.md).

| Case | Branch and required qualification |
|---|---|
| NAME-01.C01 | Unused valid name/local live unnamed target → one binding/backpointer. **Qualification:** Typed namespace/protocol mapping unresolved; no partial mutation. |
| NAME-01.C02 | Occupied name, named/dead/invalid target, invalid name → failure without changed binding. **Qualification:** Competing registration oracle; shared port namespace only if ports admitted. |
| NAME-01.C03 | whereis then death/rebind → snapshot, no reservation. **Qualification:** Subsequent PID action independently resolves; no service-handle retargeting. |
| NAME-01.C04 | Monitor name then unregister/rebind → original lifetime observed, named object reported. **Qualification:** Target incarnation independent of current registry. |
| NAME-01.C05 | Another process unregisters → allowed; invalid/absent fails. **Qualification:** No owner-only rule misrepresented as BEAM default. |
| NAME-01.C06 | Exit races unregister/reuse → old cleanup cannot remove replacement binding. **Qualification:** Revalidate current backpointer under lifetime synchronization. |
| NAME-01.C07 | Named target DOWN → old binding released; third-party replacement allowed. **Qualification:** Named-down regression with/without competing registrar. |
| NAME-01.C08 | Registry contention plus kill → no deadlock, dangling record or stale binding. **Qualification:** Lock/lifetime strategy and test proof outstanding. |

**System-task, code-lifetime and routing refinements:** [report 52](../research/52-name-resolution-and-typed-routing.md). These are planned cases; source-only, executed reference and Rust compiler evidence remain separate. Optional naming, native jobs and code replacement do not become initial requirements.

| Case | Branch, required treatment and acceptance evidence |
|---|---|
| NAME-01.C09 | Send to absent atom name versus `{Name, local_node}` and dead PID; repeat after unregister. Preserve the supported address-form failure/discard distinctions. Forbid one generic missing-target rule for every addressing form. Use report53's ordered local controls; remote distribution remains separate. |
| NAME-01.C10 | Resolve a typed name, then rebind to another incarnation/protocol before use; include deferred named timer delivery. Validate the selected binding or enforce a declared stable protocol policy. Forbid unchecked payload reinterpretation, stale type authorization and implicit per-type duplicate namespaces. Mismatch API remains unresolved; require constructor/variance and rebind controls before enabling the extension. |
| NAME-01.C11 | Enumerate empty/nonempty registry, unregister after obtaining result, and race enumeration with churn. Return owned valid names with the qualified snapshot consistency. Forbid dangling entry borrows, assumed sorted order or continuing target liveness. Verify copied-result lifetime and writer progress; compare concurrent histories against the declared snapshot interval. |
| NAME-01.C12 | Compete to register two names for one target, and one name for two targets, particularly across different proposed shards. Forbid a successful pair violating either uniqueness direction or inconsistent target backpointer. Single-key map insertion alone is insufficient. Reference races are observed; a sharded Rust transaction still needs modeling. |
| NAME-01.C13 | Retain compact name tokens in snapshots/timers while unregistering and recycling registry storage, including another runtime with identical numeric tokens. Forbid old-name-to-new-spelling aliasing and cross-runtime resolution. Qualify interning/retirement or nonreused identities; no fixed token width or allocator chosen. |
| NAME-01.C14 | Optimize repeated name routing with cached typed destinations while replacement and concurrent send occur. Distinguish an explicitly captured PID from a name resolved per operation. Forbid changing dynamic-name semantics to sticky first-owner delivery. Validate binding version/type and record lifetime as one protocol; forced invalidation races need model/negative controls beyond sequential report53. |

## OBS-01 — Ordered inspection and process enumeration

**Applicability:** Minimal correct observation initially; broad per-item compatibility and enumeration are extensions.

**Build work:** Distinguish fast inspection, ordered request and absent/exiting paths. Define each returned item and snapshot boundary; handle request death and abandoned responses. Specify snapshot enumeration separately from weaker incremental iteration.

**Dependencies:** EXEC-06, LIFE-01/05, MSG-04, EXEC-02/03, SIG-01. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** Liveness is causal and immediately staleable. Inspection never requires application cooperation, misreports its own temporary work as application state, or invokes unqualified payload callbacks. Large observations remain accounted.

**Source and limits:** [Report 21](../research/21-process-inspection-control-deep-dive.md); [43 — snapshot and observer ownership](../research/43-process-enumeration-and-observer-lifetime.md). Broad snapshot/trace APIs remain conditional; initial identity and queued-event ownership guards apply independently. New cases are source-derived plans, not executed Bream tests.

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
| OBS-01.C09 | **Later/reference snapshot:** spawn/delete/reuse a slot before/after the cut and before/after its chunk scan. Return precisely the cut's IDs once. **Qualification:** Return order is not the oracle; a returned ID may now be dead. Forbid post-cut replacement inclusion or losing the pre-cut incarnation. [S03](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L888-L954), [S05](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L956-L1044), [S07](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L1047-L1202), [T01](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L2897-L3032). |
| OBS-01.C10 | **Later/reference overlapping snapshots:** pause at least three observers, churn, resume and kill them in non-creation order. Preserve each survivor's history and reclaim only unneeded prefixes. **Qualification:** Reclamation can be deferred under tracked ownership. Forbid one observer consuming another's evidence or freeing a shared suffix. [S07](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L1047-L1202), [S08](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L795-L884), [T02](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L4696-L4744), [T03](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L4745-L4775), [T04](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L4778-L4806). |
| OBS-01.C11 | **Later/reference abandoned continuation:** owner dies during initialization, table scan, history scan or result construction, with many deletion nodes and invocation markers. Release or transfer every cleanup obligation once. **Qualification:** No exact time bound is asserted; separately record lock and turn work. Forbid mandatory cleanup depending on executing application `Drop`, or declaring reduction charging covers allocator/destructor latency. [S08](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L795-L884), [S09](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L42-L57), [S10](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L1205-L1273). |
| OBS-01.C12 | **Initial ownership guard:** an observer retains IDs/metadata while the target is killed and its slot reused. `Down` still follows stopped/unaddressable computation; future lookup validates the incarnation. **Qualification:** Historical metadata may survive; it grants neither a process lease nor access to reclaimed application state. Forbid retaining the application heap merely because an observation record is retained. [S01](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L5580-L5600), [S04](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L644-L734), [S26](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14429-L14470), [S27](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14781-L14794); accepted Bream lifecycle contract. |
| OBS-01.C13 | **Later/reference weak iterator:** a stable process spans the full traversal; other processes appear/disappear between scans and cached-item returns. Include the stable process and finish through empty chunks. **Qualification:** Transients may not form a coherent snapshot; cached IDs may be dead. Forbid advertising snapshot semantics or treating a yielded ID as a liveness guarantee. [S11](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L5602-L5666), [S12](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L1458-L1539). |

## OBS-02 — Process diagnostics, sensitivity and event lifetime

**Applicability:** Later diagnostic APIs; basic counters continue in EXEC-06.

**Build work:** Separate lifetime monitors from runtime diagnostic events. Model trace sessions, event recipients, sensitive-data suppression, saved-call state and mailbox threshold hysteresis; inventory event copy/formatting cost.

**Dependencies:** OBS-01, EXEC-06, MSG-01/04, LIFE-05. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** State what each event proves and what it does not. Test recipient/session teardown and overhead; diagnostics cannot silently become enforcement, security isolation or a new process-admission escape.

**Source and limits:** [Report 21](../research/21-process-inspection-control-deep-dive.md); [43 — snapshot and observer ownership](../research/43-process-enumeration-and-observer-lifetime.md). Broad snapshot/trace APIs remain conditional; initial identity and queued-event ownership guards apply independently. New cases are source-derived plans, not executed Bream tests.

| Case | Branch and required qualification |
|---|---|
| OBS-02.C01 | Toggle sensitivity around tracing/inspection; suppress specified data, preserve state, and separately test labels and earlier saved events. |
| OBS-02.C02 | Set call-history capacity to zero/nonzero/out-of-range, locally or on dying target; preserve prior-value/reset/error behavior and release request copies. |
| OBS-02.C03 | Mailbox crosses enable/disable thresholds repeatedly; emit hysteresis transitions without imposing mailbox capacity. |
| OBS-02.C04 | Diagnostic receiver dies or trace session changes while events are in flight; no dangling receiver, leaked message or stale-session effect. |
| OBS-02.C05 | Long scheduling/GC and repeated runnable transitions; distinguish diagnostic events from enforcement and from actual scheduling-turn counts. |
| OBS-02.C06 | Tracing/inspection creates substantial work or invokes a native callback; qualify overhead and cleanup without compromising admitted-process guarantees. |
| OBS-02.C07 | **Later/reference session ownership:** retain only weak IDs/tracee attachments, then remove all strong public and temporary work holds. Initiate cleanup without requiring tracee death. **Qualification:** Cleanup may be asynchronous. Forbid making attachment ownership prevent automatic cleanup, or freeing before non-owning attachments are cleared. Test retained strong handles separately. [S13](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/kernel/src/trace.erl#L1403-L1455), [S17](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_trace.c#L1188-L1227), [S19](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_trace.c#L605-L646), [S20](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L3621-L3743), [S32](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L2432-L2464), [S34](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L2683-L2693), [T05](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/trace_session_SUITE.erl#L1636-L1718). |
| OBS-02.C08 | **Later/reference destroy phases:** destroy an active session; kill the requesting process while waiting for the barrier; destroy again using a retained valid handle. Cleanup remains owned and the second destroy reports inactive. **Qualification:** Already-sent events may remain; physical retirement can follow return. Forbid resuming the dead requester, conflating barrier completion with all reclamation, or draining unrelated receiver messages. [S13](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/kernel/src/trace.erl#L1403-L1455), [S15](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_trace.c#L1279-L1348), [S18](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_trace.c#L535-L601), [S19](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_trace.c#L605-L646). |
| OBS-02.C09 | **Later/reference cleaner race:** target dies before enqueue, after accepted enqueue, or during cleanup; another session is attached meanwhile. Failed send adds no pending request; accepted request acknowledges after cleaning its batch, including on the exiting path. **Qualification:** New unrelated attachments survive. Forbid an acknowledgment debt for failed send, premature batch retirement, or unaccounted variable work presented as bounded. [S21](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erts_trace_cleaner.erl#L29-L100), [S22](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_trace.c#L650-L725), [S23](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L3380-L3431), [S25](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7400-L7416), [S28](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L3563-L3621), [S29](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6963-L6971). |
| OBS-02.C10 | **Later/reference tracer death:** default tracer dies between status check/event delivery; repeat from self/other/dirty contexts and replace the tracer. Disable/discard appropriately and remove references only through qualified contexts. **Qualification:** Physical removal may be lazy; session destruction is a separate operation. Forbid reusing the dead tracer's state for new tracing or assuming an ordinary event check immediately clears all refs. Dirty/custom-native execution remains an unclosed branch. [S30](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/nifs/common/erl_tracer_nif.c#L121-L149), [S31](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L3285-L3374), [T06](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/trace_SUITE.erl#L1922-L1951), [T07](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/trace_call_time_SUITE.erl#L545-L583). |
| OBS-02.C11 | **Initial event-ownership guard; full system-monitor API later:** queue an owned event, destroy its observer session or replace/kill its receiver before delivery. Deliver/drop according to the supported routing contract and release payload/session ownership once. **Qualification:** A queued event may outlive logical observer disablement. Forbid stale receiver failure clearing replacement configuration, use after session retirement, or extending application execution to satisfy observation. [S32](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L2432-L2464), [S33](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L2740-L2858), [S34](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L2683-L2693), [S35](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_trace.c#L2530-L2562). |

## IO-01 — Process I/O routing and native resource boundary

**Applicability:** Runtime-aware I/O is accepted but follows the local process slice. Native-call admission remains an initial safety obligation.

**Build work:** Define inherited I/O routing separately from links. For each admitted handle, model readiness registration/rearm, notification detachment, cancellation, stop acknowledgment, safe close and generation reuse. Keep the runtime owner alive until backend access ends. Qualify each dependency call transitively.

**Dependencies:** LANG-01/03, EXEC-02/03/05, LIFE-05, MSG-04, RES-01. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** Test dead recipients, reused descriptors, deferred close, failures and shutdown. Neither a cancellation result nor a death notification is automatically a safe-close acknowledgment. No NIF/driver ABI or bundled algorithm port is required.

**Source and limits:** [Report 22](../research/22-process-io-native-boundary.md). Follow-ups: [42 — Linux backend](../research/42-linux-readiness-and-registration-lifetime.md), [44 — native resource lifecycle](../research/44-native-resource-callback-and-owner-lifetime.md) and [45 — isolated Linux observations](../research/45-linux-io-observations-and-rust-boundary.md). C09–C16 preserve conditional native-resource reference branches; arbitrary shared mutable NIF resources, callbacks and code replacement are not added to initial admission. C17–C25 refine backend integration. All remain planned; the syscall probes support only selected Linux facts.

| Case | Branch and required qualification |
|---|---|
| IO-01.C01 | Spawn inherits I/O routing; change routing on self/live/busy/exiting target; preserve signal ordering and distinguish successful assignment from leader liveness. |
| IO-01.C02 | Register/rearm read/write interests, replace recipient/reference or encounter another owner's registration; define ownership and prevent stale-generation delivery. |
| IO-01.C03 | Readiness is pending, detached, delivered or consumed when cancellation occurs; cancelled flags must match the actual boundary, without flushing unrelated mailbox items. |
| IO-01.C04 | Stop callback can run immediately or only after poll coordination; do not close/reuse the OS handle before safe closure. |
| IO-01.C05 | Recipient dies with registrations or detached notifications outstanding; release notification ownership and eventually stop native registrations without post-death application execution. |
| IO-01.C06 | Registration allocation/platform/poll failure; establish the actual post-failure interest and ownership state, release abandoned work and distinguish unsupported capability from successful empty readiness. Do not presume every backend failure leaves the previous registration unchanged; refined error/fallback branches are in IO-01.C18. |
| IO-01.C07 | Resource callback/destructor or an ordinary Rust dependency performs unbounded work; fail admission or qualify a resumable path, never count dirty-worker offload as safe kill. |
| IO-01.C08 | Runtime shutdown races readiness/cancel/stop and descriptor reuse; retain required memory until the final owner completes, and expose truthful cleanup completion. |
| IO-01.C09 | Register without down capability, with undefined/dead target, during target exit and twice against one target. Check failure rollback, independent identities and publication-versus-return ordering; forbid callback state that is uninitialized when publication becomes visible. Sources: [registration](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3933-L3989), [target late handling](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7347-L7360). Early callback timing remains an unexecuted probe. |
| IO-01.C10 | Cancel using wrong resource, repeated identifier, or while the firing path claims origin. Zero suppresses that monitor's future callback but need not reclaim target-side state immediately; nonzero is not callback completion. Forbid treating failed cancellation as a join or successful cancellation as cancellation of another monitor. [Cancel](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3998-L4030); [race contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L1159-L1185). |
| IO-01.C11 | Race last strong resource release with origin lookup/conditional callback hold and DYING publication. Accept the source branches: callback with a valid hold, or suppression without resurrection. Forbid destructor overlap with a running held callback, double endpoint release or stuck monitor counts. [Fire](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3059-L3118); [dtor](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L2961-L3029). |
| IO-01.C12 | The monitored process holds the last Erlang resource term; compare another process/native owner retaining a handle. Record PID removal, ordinary process DOWN, resource callback and final destruction separately. Forbid assuming creator death uniquely owns or destroys shared resource storage, or using process DOWN as a cleanup barrier. [Strong ownership](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3143-L3222); [exit order](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14545-L14594); [self-monitor test](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/nif_SUITE.erl#L1442-L1481). |
| IO-01.C13 | Multiple monitored targets exit concurrently; callback cancels another monitor or requests direct stop; callback allocates/sends terms. Qualify mutable-state synchronization and nested callbacks without holding incompatible locks. Forbid escaping temporary environments, assuming per-resource callback serialization, or arbitrary callback admission based solely on `Send`. [Fire](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3059-L3118); [direct stop](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/common/erl_check_io.c#L1485-L1538); [environment](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L573-L587). |
| IO-01.C14 | One resource has multiple event relations and native/term owners; stop one relation directly and another asynchronously while owners disappear. Destructor is last; each registration hold survives its stop return. Forbid closing/reusing an event through ordinary field Drop before safe stop, or attempting resource resurrection from the destructor. [Relation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L3577-L3608); [stop release](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/common/erl_check_io.c#L2182-L2247); [dtor contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L738-L772). |
| IO-01.C15 | Last strong release detaches a large monitor tree; interleave target death, demonitor acknowledgments, late target signals and final protocol-reference release. Retain resource storage until settlement; measure the unsliced walk and nested cleanup separately. Forbid lost cleanup ownership, duplicate destructor scheduling effects or claiming local count accounting is a latency bound. [Dtor walk](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L2961-L3029); [ack](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3042-L3056); [late signal](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7347-L7360). |
| IO-01.C16 | Purge/replace resource callback code while resources remain, including a down-only type with no user destructor; alternatively exercise an explicitly static Rust resource implementation. Forbid callback into unloaded code or inferring no callbacks from an absent user destructor. Dynamic takeover remains unsupported unless independently qualified. [Normalization](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L2746-L2763); [purge](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L5334-L5372); [regression](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/nif_SUITE.erl#L1485-L1533). |
| IO-01.C17 | Concurrent FD-table growth, sparse high FD and rejected bounds **Qualification:** Fully initialized stable registration lookup across growth; no cached pointer into moved state or falsely successful invalid/internal registration. Measure global lock pause. |
| IO-01.C18 | ADD rejected then fallback; MOD fails then deletion; DEL errors; repeated EINTR **Qualification:** Establish exact poststate and owned-resource accounting with fault injection. Never treat all errors as a rollback, empty readiness or automatic retry of the same operation. |
| IO-01.C19 | Read/write readiness arrives partially or after cancellation; remaining directions rearm; FD reused or duplicated **Qualification:** Preserve one pending notification per qualified request and tolerate stale/spurious readiness followed by EAGAIN on the current registered object. Old notification payload/callback ownership must not authorize access to a replacement incarnation. No BEAM generation-token guarantee is assumed. Test failed rearm and distinguish backend one-shot from public one-shot. |
| IO-01.C20 | ERR/HUP with/without explicit error interest, then cancel/rearm/STOP **Qualification:** Follow current flags and masks, including sticky WANT_ERROR history; no invented unrequested notification, dropped unread bytes or HUP-as-immediate-EOF assumption. |
| IO-01.C21 | Fallback DEL acknowledged by NONE; repeated or conflicting call while STOP_NIF pending **Qualification:** Distinguish control acknowledgment from readiness; callback/release occurs at its valid boundary. Defensive misuse branches do not authorize early descriptor reuse. |
| IO-01.C22 | Replace pending notification; custom environment transfer versus term copy; detach before cancellation/death **Qualification:** Each envelope/resource ownership moves or releases once; no reuse of a published envelope and no retained old recipient after successful replacement of a still-pending request. |
| IO-01.C23 | Same/different recipient around migration threshold; scheduler slots exhausted; rearm, failed delivery or deselect **Qualification:** Correct requested/active/in-flight states and outstanding count; no old scheduler entry clearing a new notification or stranded input across pollsets. |
| IO-01.C24 | Coalesced wakes, full wake pipe, EINTR, timer-only results and premature zero-result wait **Qualification:** No lost control work or application notification for internal FDs; preserve deadline across retry and distinguish EAGAIN/EINTR/timeout exits. |
| IO-01.C25 | Poll result batch fills repeatedly while cold I/O, timers and process death compete **Qualification:** Account retained batch work and progress, qualify growth/caps and cleanup latency; throughput alone cannot certify fairness or forced-termination responsiveness. |

## LANG-04 — Catchable failures and lifecycle handoff

**Applicability:** Initial mapping obligation alongside checked code and exit semantics.

**Build work:** Specify catchable operation errors, explicit process exit, throw/rethrow if admitted, uncaught failure and Rust panic separately. Track reason/trace ownership across suspended helpers. Define diagnostic failure and host-fatal boundaries.

**Dependencies:** LANG-01/02/03, EXEC-04, LIFE-04/05, MSG-02. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** Caught errors resume only valid owned state; uncaught failure commits one terminal transition; direct runtime kill cannot be caught. Explicitly reject unsupported language forms instead of pretending Rust panic is a complete BEAM exception model.

**Source and limits:** [Report 23](../research/23-process-exceptions-code-lifetime.md).

| Case | Branch and required qualification |
|---|---|
| LANG-04.C01 | A supported operation raises an error that is caught; continue with valid owned state and correct exception class/reason, without turning it into runtime kill. |
| LANG-04.C02 | Explicit exit is caught or uncaught; direct kill arrives during catching; preserve the separate lifecycle behavior and the untrappable forced-stop guarantee. |
| LANG-04.C03 | An uncaught throw/error or supported rethrow ends execution; map reason and trace metadata without leaking retained payloads or claiming ordinary Rust panic equivalence. |
| LANG-04.C04 | Exception handling/tracing crosses suspended helper state or fails during diagnostics; release each ownership once and retain the declared host-fatal boundary. |

## CODE-01 — Executable-code lifetime and purge boundary

**Applicability:** Later research only; static linked Rust code may exclude replacement entirely.

**Build work:** If dynamic replacement is proposed, enumerate direct instruction/return/native-wrapper references, code inspection, suspension and release. Separate soft refusal from hard termination and code bytes from ordinary data references.

**Dependencies:** LANG-03/04, LIFE-05, MEM-01, OBS-01. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** No code storage can be freed while any admitted continuation may execute it. Establish the native lifetime contract before implementing purge; this report is not a hot-upgrade design.

**Source and limits:** [Report 23](../research/23-process-exceptions-code-lifetime.md).

| Case | Branch and required qualification |
|---|---|
| CODE-01.C01 | Check code with missing module/no old version, current instruction, saved return address or native wrapper reference; distinguish direct executable references from ordinary data references. |
| CODE-01.C02 | Code inspection races execution, dirty work, death or delayed system tasks; no reading or reclaiming a still-used continuation without the proper ownership barrier. |
| CODE-01.C03 | Soft/hard replacement sees a live old-code user; refuse or terminate according to an explicit contract, and observe kill completion before unsafe reclamation. |
| CODE-01.C04 | Old closures, literal storage, exception traces or diagnostic locations outlive code replacement; reject unsupported native upgrades or specify lifetime and failure behavior before exposing them. |

**System-task, code-lifetime and routing refinements:** [report 51](../research/51-code-literal-retirement-and-rust-lifetimes.md). These are planned cases; source-only, executed reference and Rust compiler evidence remain separate. Optional naming, native jobs and code replacement do not become initial requirements.

| Case | Branch, required treatment and acceptance evidence |
|---|---|
| CODE-01.C05 | A fun invocation races purge preparation and then soft-purge abort. Publish the pending-entry state before inspection; restore dispatch and complete its visibility boundary before clearing suspension state. No new old-code entry after the inspection boundary and no caller stranded by abort. [source S02](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_fun.c#L159-L240), [source S03](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_bif_load.c#L2100-L2219), [source S05](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erts_code_purger.erl#L74-L103) |
| CODE-01.C06 | Successful purge returns while another process or queued message retains literal data. Separate executable removal, queued evacuation and storage reclamation; data remains readable until its own ownership protocol completes. A Rust static configuration rejects replacement without invalidating existing constant/message values. [source S04](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_bif_load.c#L2248-L2326), [source S09](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L8146-L8289), [source R01](https://doc.rust-lang.org/reference/items/static-items.html#items.static.lifetime) |
| CODE-01.C07 | Send literal-bearing messages immediately before/after the copy barrier while the receiver scan yields. Preserve each message's contents and order, attach/retain its own surviving data, and retain the scan markers. Never free the area based on an unordered snapshot or let post-barrier sends resurrect its unowned references. [source S06](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.c#L719-L797)–[source S10](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L8293-L8338) |
| CODE-01.C08 | References survive only in a matched-message fragment, abandoned heap, dictionary or direct argument register; repeat during GC-disabled construction and dirty work. Include all admitted holders, defer unsafe inspection, and distinguish a pending check from completion. A conservative unnecessary collection is safer than following stale register pointers. [source S11](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_bif_load.c#L1006-L1114)–[source S13](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L11128-L11179) |
| CODE-01.C09 | A small live literal belongs to a large area; evacuation crosses a configured heap limit, or the holder was hibernated. Account temporary and retained storage, preserve the declared failure policy and restore the applicable suspended state. Do not report copy volume, heap charge and final live bytes as one number. [source S14](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1160-L1246), [source S18](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1375-L1389), [source S19](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1197-L1210), [source T04](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/code_SUITE.erl#L1455-L1486) |
| CODE-01.C10 | More holders require GC than the allowed concurrent jobs; initial replies, queued jobs and unrelated/stale-area replies interleave. Maintain area/request correlation and total unfinished work; no advance while a holder remains unprocessed. Correctness must survive a different qualified Rust work limit. [source S22](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erts_literal_area_collector.erl#L27-L159) |
| CODE-01.C11 | A terminating process replies while a non-immediate reason or dirty helper can still access literals, including an area queued later. Acquire the blocker before the reply and release it only after the last covered access. Death notification or task acknowledgment alone cannot authorize reclamation. [source S20](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14370-L14402), [source S21](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13445-L13481) |
| CODE-01.C12 | Back-to-back areas rotate counters while a terminating process stalls; race its last unblock with collector waiting and a newly queued area. Preserve wakeup, counter-generation identity and the visibility boundary; allow useful work on the next area without prematurely freeing the previous one. [source S23](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_bif_load.c#L1810-L1946), [source S24](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_bif_load.c#L1693-L1723) |
| CODE-01.C13 | Several holders retain a binary/sub-binary after its original area retires; some holders die during transfer and one survives. Establish each backing owner before releasing the old one, release each obligation once and keep the surviving view valid. Final wrapper removal does not imply final backing allocation release. [source S16](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1323-L1373), [source S17](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_load.c#L683-L716), [source T03](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/code_SUITE.erl#L973-L1030) |

## SIG-01 — Control dispatch through process death

**Applicability:** Initial exhaustive handling for every admitted internal control variant; unsupported extension variants remain out of the implementation.

**Build work:** Maintain a finite control-kind inventory with live/exiting/dead actions, correlation owner and release path. Distinguish notification conversion, terminal replies, relation acknowledgment, barrier completion and discard. Internal callbacks require a qualified contract, not arbitrary user closures.

**Dependencies:** MSG-04, LIFE-01/02/03/04/05, EXEC-02/03, OBS-01, IO-01. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** Every admitted kind has explicit response/drop/cleanup outcomes around death and requester loss. Generate coverage checks from the Bream variant list; the 21-kind BEAM dispatcher inventory is a reference map, not a requirement to copy every opcode.

**Source and limits:** [Report 24](../research/24-process-signal-dispatch-matrix.md).

| Case | Branch and required qualification |
|---|---|
| SIG-01.C01 | Every admitted control kind arrives before/during/after exit transition; define response/drop/cleanup for each, without resuming application work. |
| SIG-01.C02 | A requester's death races a runtime RPC/inspection/suspension result; release request and reply ownership and avoid indefinite pending obligations. |
| SIG-01.C03 | Internal signal flush crosses exit, partial queue handling or delayed conversion; distinguish barrier completion from application-message removal. |
| SIG-01.C04 | Persistent event has active/removed subscription, custom tag or repeated node observation; preserve the correct conversion multiplicity and lifetime if exposed. |
| SIG-01.C05 | Expensive adjustment/control callback uses or extends its work allowance; qualify progress, stop boundaries and partial-state cleanup instead of assuming the dispatch count is a hard limit. |

## RES-01 — Process-owned external state and ownership transfer

**Applicability:** Reference and later integration work. Shared ETS-like mutable state and port APIs are outside the accepted initial state model.

**Build work:** Study owner-death deletion, heir validation, explicit transfer and resumable release as resource protocols. For native I/O distinguish connected ownership from links. If shared tables are ever proposed, first resolve the conflict with accepted process-owned state; do not port table/query algorithms now.

**Dependencies:** EXEC-03, LIFE-01/03/05, MSG-04, IO-01. These are integration obligations; [G0–G7](../build-plan.md) still determines incremental build order.

**Acceptance:** An ownership transfer has one valid recipient or a defined deletion path. Process death and interrupted cleanup must not orphan external state. Driver shutdown remains separately qualified; no automatic adoption of ETS or ports follows.

**Source and limits:** [Report 25](../research/25-process-owned-external-state.md).

| Case | Branch and required qualification |
|---|---|
| RES-01.C01 | Last table reference disappears with owner alive; separately owner dies without a usable heir. First table remains; second enters deletion. **Qualification:** Forbid reference-count lifetime masquerading as owner lifetime. Check table visibility before/after the owner exit with a retained test identifier. |
| RES-01.C02 | Owner dies with live heir, with and without heir data. Ownership transfers in both; only the data form sends `ETS-TRANSFER`. **Qualification:** Forbid requiring message consumption to commit ownership, or synthesizing notification for the silent form. Check access and mailbox separately. |
| RES-01.C03 | Heir absent at configuration, dies later, equals exiting owner, or PID is reused. No transfer to an unrelated incarnation. **Qualification:** Forbid resurrection or transfer by numeric PID alone. Exercise dead/reused identity fixtures; concurrent death permits transfer-then-cleanup or failed transfer followed by deletion. |
| RES-01.C04 | Owner gives away to live different local PID; contrast self/dead/non-owner/invalid table. Success transfers and preserves heir; invalid operations fail. **Qualification:** Forbid partial ownership changes on failure or deleting a successfully transferred table when the former owner dies. Verify transfer-back and heir round trip. |
| RES-01.C05 | Kill owner or authorized non-owner during large-table deletion; also exit with fixations on another owner's table. Cleanup continues with retained responsibility. **Qualification:** Forbid orphaned destruction, inheritance of a deleting table, or permanent dead-process fixations. Observe cleanup rescheduling and eventual resource return; do not equate one yield with bounded latency. |
| RES-01.C06 | Reconnect a port, then deliver old/new owner linked exits; compare BIF versus message connection and output-draining versus `kill` closure. **Qualification:** Forbid treating connection and link sets as identical or promising external-program death. Separate metadata/link checks, accepted exit outcomes and native cleanup completion; race with close permits documented failure. |

**Worker service and async-lifetime refinements:** [report 55](../research/55-port-tasks-async-jobs-and-shutdown.md). All cases remain planned. Isolated Rust measurements and models do not execute these BEAM branches or select the conditional controller/native-job APIs.

| Case | Branch, required treatment and acceptance evidence |
|---|---|
| RES-01.C07 | Pause port-task publication before lookup retention, admission, insertion and activation; race exit, `nosuspend` busy rejection, execution and explicit abort. No task both executed and aborted, no unowned rejected payload, no freed queued node, and no interpreting scheduling success as operation success. Record exactly one settlement and final release across each claim winner. |
| RES-01.C08 | Sender A has a busy-waiting command then a signal-dependent control operation; sender B has independent work. Toggle busy state and trigger `nosuspend` cancellation. No dependent A operation overtakes its predecessor; do not invent total order between independent senders or leave a canceled command's queue-byte accounting. Trace caller dependency and settlement separately. |
| RES-01.C09 | Close with tasks in busy, local and incoming queues, including already-aborted nodes, stale readiness and suspended senders; pause any proposed cleanup cursor between queues/tasks. Preserve required busy/local/incoming abort order, exactly-once stale-event cleanup and waiter release; never acknowledge reclamation with residual task ownership. Measure the whole nested cleanup path rather than only the outer turn budget. |
| RES-01.C10 | Complete an async request while its port is live, closing/draining, dead or missing a ready callback; kill before native invocation and separately while it runs. Reference queued/running invocation may continue; live/closing ready delivery or free-path ownership must settle once with code/PDL still retained. Do not turn suppressed delivery into stopped execution or allow admitted Bream process/helper work after `Down`. |
| RES-01.C11 | Submit numbered requests with equal keys, colliding different keys and unkeyed routing; vary submitting scheduler while holding one completion consumer back. Check required native invocation sequence separately from ready-callback sequence; do not assume equal-key completion FIFO across scheduler queues without evidence. Probe the source-permitted reordering candidate with controlled barriers. |
| RES-01.C12 | Produce a ready burst with cheap and expensive callbacks/free functions, port-lock contention and dead recipients. Every accepted completion retains one owner and settles once; a 20-callback cap is not a service-time bound. Measure publication contention, consumer progress and maximum uninterrupted callback/cleanup work. |
| RES-01.C13 | Seal shutdown with queued jobs, one in-flight job, ready results and a closing resource awaiting completion; race attempted new admission. No post-shutdown callback into released runtime state, no lost accepted record and no join completion claimed before native return. Keep VM `_exit` escape separate from safe embedded shutdown and preserve stop-before-`Down` for admitted process work. |
