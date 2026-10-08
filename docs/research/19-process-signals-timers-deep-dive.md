# Process signal transport, priority receive and timers: deeper source audit

**Execution follow-up:** [Report 29](29-pinned-reference-observations.md) now records targeted observations on the exact pinned build. The source-only status and hypotheses below describe the original review stage; the follow-up identifies which variants were executed and which remain unresolved.

Inspected **2026-10-07**, pinned Erlang/OTP **OTP-29.1.1**, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`, in `/home/bat/projects/bream/upstream/otp`. The checkout was read only. **No runtime, test suite, experiment or generated fixture was executed.** This report extends [12](12-beam-mailboxes-source.md) and [15](15-mailbox-edge-cases.md); it does not turn source inspection into an exhaustive concurrency proof.

The expanded subject is directly process-related machinery: local signal publication, aliases, priority reception, receive markers, receive timeouts and BIF message timers. Application libraries such as compression or cryptography are excluded. Distribution is inspected only at dispatch boundaries where it changes a local API's meaning. Monitor/alias relationship cleanup belongs to [report20](20-process-lifecycle-deep-dive.md); scheduling, GC and suspension integration belongs to [report18](18-process-execution-memory-deep-dive.md).

## Coverage and confidence

“Traced” means the named main call path and relevant branches were followed in source, not that every reachable helper or interleaving was proved. “Sampled” means selected helpers/tests were inspected. All findings remain unexecuted.

| Area | Depth in this report | Still outside the claim |
|---|---|---|
| Ordinary local message/control publication, outer/middle/inner queues | Traced direct, buffered, pending-monitor, fetch and notify branches | Complete memory-order proof; every tracing/dirty-scheduler path |
| Alias delivery | Traced active/absent/pending/one-shot alias handling and priority gating | Full monitor relationship protocol and distributed alias encoding |
| Priority receive | Traced insertion, cursor redirection and capability-count removal; sampled term walker/tests | Every marker combination; worst-case traversal cost |
| Receive markers | Traced reserve/bind/use/clear/reuse/pass and deferred middle-queue position | Complete compiler data-flow correctness; all JIT backends |
| Receive timer | Traced arming, cancellation, expiry flag, scheduler continuation and pause/resume | Exhaustive match/expiry/publication/exit interleavings |
| `start_timer`/`send_after`, read/cancel | Traced options, recipient attachment, expiry CAS, local/remote scheduler access, death cleanup | Complete timer-wheel implementation and canceled-queue lifetime proof |
| Suspended process timers | Traced pause/rebuild/counting; sampled upstream tests | Original timer-reference behavior needs an exact-version fixture |

## 1. A signal is not already a received mailbox message

The private-queue description distinguishes **inner** messages/markers from **middle** messages and control signals; incoming signals first live in the **outer** queue or its buffers. Outside signal handling, only the inner queue corresponds to received messages. During handling, the processed middle prefix also counts. Physical publication, signal reception, application selection and message-body execution are different events. The old comment mentions a `len` field, while the actual structure has `mq_len` and `mlenoffs`; use the current field operations rather than treating that historical prose as a counter specification. [Queue structure][queues].

Ordinary local dispatch is `do_send` → `erts_send_message` → `erts_queue_proc_message` → `queue_messages`. Alias/priority dispatch is `do_send` → `erts_proc_sig_send_altact_msg` → `proc_queue_signal` → `handle_altact_msg`. The latter stays a non-message signal until receiver-side policy resolves it. [Ordinary dispatch][send-dispatch], [alternate construction][alias-create], [alternate handling][alias-handle].

| Branch / invariant | Pinned implementation evidence and consequence |
|---|---|
| Receiver absent at lookup | Ordinary PID send returns normally; alternate send also returns without publication. This is not a delivery acknowledgment. [Ordinary dispatch][send-dispatch]; [alternate lookup][alias-create]. |
| Ordinary receiver becomes exiting | `queue_messages` checks state under MSGQ and cleans up messages. General control publication instead rejects FREE state; an exiting record may still need system protocol work. Do not unify these into “drop every signal on exiting.” [Ordinary drop][ordinary-drop]; [control publication][signal-publish]. |
| Scheduler holds a pending process-monitor signal | `proc_queue_signal` may defer the monitor; a later same-destination control signal prepends it. The ordinary message path similarly prepends the pending signal. Failure can synthesize monitor-down with `noproc`. Optimization must preserve cross-kind order. [Pending signal][pending]; [message prepend][prepend]. |
| Direct publication with off-heap buffers present | `enqueue_signals` flushes buffers before appending to the main incoming queue. A direct path must not overtake its sender's buffered history. [Enqueue flush][enqueue-flush]. |
| Buffered publication | Sender identity hashes to one slot; its lock preserves sender order. A dead/uninstalled buffer causes fallback. Different slots do not create a global send order. [Buffer selection][buffer-select]. |
| Buffer flush races enqueue | Slot flags use a restart/recheck protocol; flush exchanges the nonempty bitmap and concatenates locked slot queues. Fetch repairs incoming-signal flags if publication raced flag clearing, including scheduling another notification. [Slot race][buffer-race]; [flush][buffer-flush]; [fetch repair][fetch-repair]. |
| Fetch has only ordinary messages | With no older middle queue and no receive tracing, fetch links directly into the inner queue. Otherwise it appends to middle and maintains non-message links/message offsets. Fast-path equivalence depends on those preconditions. [Fetch branches][fetch]. |
| Empty scan races publication | Receive helper retains the queue lock across the decision to wait. First-buffer publication waits for MSGQ release before notification so sleep cannot overwrite wakeup. Flags can conservatively report work until a later fetch; a flag is not itself a user mailbox item. [Wait handoff][wait-handoff]; [buffer wakeup][buffer-wakeup]. |

The scalar signal budget remains operation-boundary accounting. Weighted operations execute before the loop tests its allowance; nominal reduction limits are not exact cost or latency caps. A Bream queue must budget its own copying, conversion, reference walking and teardown rather than copying ERTS constants. [Post-operation limit][signal-limit].

## 2. Alias deactivation filters delivery; it does not flush received messages

The documented boundary is precise: deactivated-alias messages are discarded **before entering the mailbox**; messages already received remain. Only the creator can deactivate an alias, and the public interface does not offer alias lookup/liveness inspection. Thus successful sending to a reference does not disclose whether it currently denotes an active alias. [Alias contract][alias-doc].

`erts_proc_sig_send_altact_msg` extracts a candidate PID from the reference, copies the payload and metadata into heap/off-heap/fragment storage, and queues an `ALTACT_MSG` non-message signal. If publication fails, it cleans that representation up. Sender-side successful publication has not yet accepted the alias. [Construction and ownership][alias-create].

`handle_altact_msg` performs these receiver-side branches:

1. A direct PID destination cannot make an application message priority merely by passing `[priority]`.
2. An alias must have an active alias state and must not be `SPAWN_PENDING`. Missing, deactivated, never-active and not-yet-active aliases drop the signal and reclaim its message representation.
3. Priority is accepted only when both send metadata and the alias's priority flag permit it; an active ordinary alias receives ordinary delivery.
4. `ALIAS_ONCE` clears alias state and removes its monitor-tree entry **when this signal is handled**, before application selective receive. For a local monitored process it sends demonitor; standalone alias storage is released. The reply does not need to match the application's current receive for one-shot deactivation to happen.
5. Ordinary accepted content converts in place; priority content takes the separate insertion path. A one-shot priority alias's last priority-capability count is removed only **after** insertion, so its first accepted reply is still prioritized. [Receiver policy][alias-handle]; [last priority alias ordering][alias-prio-release].

Consequences for fixtures: race deactivation against publication and against receiver handling separately; verify old mailbox entries survive; send two replies through a one-shot alias while selecting neither; check only the first accepted reply enters the mailbox. With multiple senders, do not choose the first accepted reply from wall-clock timestamps. Alias/monitor cancellation races need the shared lifecycle model, not two independent implementations.

## 3. Send options do not turn local mailboxes into bounded channels

`send/3` parses `noconnect`, `nosuspend` and `priority`, rejecting unknown options or an improper option list. Ordinary local PID delivery does not use connection establishment or busy-port/distribution admission. `nosuspend` avoids the latter suspension conditions; it does not prohibit normal scheduler yielding or make large payload preparation constant-time. The successful send path still checks reductions and can yield-return. [Option dispatch][send-options].

`send_nosuspend` maps `send(...,[nosuspend|Opts])` success to `true`, other return statuses to `false`. Public documentation associates possible suspension with overloaded remote transport and busy local ports. A true result is not acknowledgment of remote arrival. `noconnect` affects the remote connection boundary; it supplies no meaningful local mailbox capacity policy. The distinct transport paths and their failure meanings must remain explicit if Bream later adds them. [Public options][send-options-doc]; [wrapper and transport contract][nosuspend-doc].

An external PID/reference from an old incarnation of this node is dropped by a dedicated dispatch branch; ordinary local registered-name failure is another separate path. Those are not reasons to redefine dead local PID send as a checked delivery API. Full distributed encoding, connection failure and port busy protocols remain unreviewed here. [Old-node dispatch][old-incarnation].

## 4. Priority reception changes insertion, not signal delivery order

Priority mailbox reception has existed since OTP 28. The receiving mailbox becomes a priority prefix followed by ordinary messages. Each class preserves its reception order, while an accepted priority message can move before earlier ordinary messages from the same sender. Signals still arrive in order. Receive always selects the earliest matching message in the resulting mailbox; application code cannot inspect a separate priority bit through ordinary receive. [Priority contract][priority-doc].

For explicit message sends, priority requires an active priority alias **and** `[priority]`. Priority monitor/link capabilities enable corresponding notification placement. For trapped exits this changes the message's insertion, not the priority of the exit signal itself. Untrappable kill is not an application message that waits behind a priority prefix. [Capability requirements][priority-enable].

The insertion algorithm is more than “push to front”:

- `insert_prepared_prio_msg_attached` inserts immediately before an end marker, after older priority messages.
- If the receive cursor already scans the priority region, it is already before the insertion point.
- Otherwise FIRST mode requires redirection; LAST mode represents an internal emulator reply wait whose fresh references are not carried by priority replies, so it can stay put.
- RCVM mode considers whether the bound reference appears in the priority prefix. Before insertion, `prio_queue_check_recv_marks` traverses the new term to mark relevant references.
- When redirection is necessary, a continuation marker saves the old ordinary position; scanning visits the new priority item, then restores that position at the priority-end marker. [Reference detection and insertion][priority-insert]; [continuation restoration][priority-continue].

This invalidates any broad claim that one pending receive never revisits an earlier physical queue segment. The narrower ordinary-mailbox cursor property from report15 remains useful. The priority term walker uses an explicit term stack and no internal yield in the inspected function; its input-sized cost needs separate qualification if borrowed. [Term walker][priority-walk].

Removing the final priority alias/link/monitor clears capability availability, but existing priority items and their end marker remain until drained. Re-enabling can reuse that still-existing prefix. Dropping the capability is not demoting or discarding already-queued messages. [Priority lifetime][priority-lifetime].

Sampled tests strengthen the branch catalogue: `priority_messages_order` checks the combined application/Exit/Down order, including `[priority]` sent directly to a PID remaining ordinary; `prio_msg_while_in_receive` delivers during a million-message scan and after the receiver is waiting; `priority_messages_qmarkers` exercises priority capability/marker changes. These test sources were read, not run. [Mixed-order fixture][priority-test]; [scan/wait fixture][priority-receive-test]; [marker suite entry][priority-marker-test].

## 5. Receive markers have an entire lifecycle

Markers are compiler/runtime proof artifacts, not user messages. Reserve runs before creation of a fresh reference so a rapid reply or immediate Down cannot arrive before the saved boundary. The compiler's reference-use analysis supplies the justification; arbitrary application IDs do not. [Compiler rationale][compiler-marker].

| Lifecycle branch | Traced mechanics and obligation |
|---|---|
| Reserve with empty/nonempty queues | `erts_msgq_recv_marker_insert` first fetches incoming signals under MSGQ. Empty queues return `undefined`; otherwise insertion occurs at the current signal end, which can be in middle rather than inner. [Reserve/bind API][marker-api]. |
| Bind to a reference or invalid value | Binding looks up the temporary marker ID. An internal reference replaces it; another term makes the slot unused and ages it out. Missing IDs are harmless no-ops. [Bind][marker-api]. |
| Use while marker still in middle | Save is positioned at the inner end to force signal processing; `pending_set_save_ix` and `set_save` delay the actual boundary until the marker reaches inner. [Deferred position][marker-use]. |
| Use after relevant priority arrival | An `in_prioq` marker requires scanning the priority prefix first; reaching its end restores the marker position or its still-pending middle-queue continuation. [Use modes][marker-use]; [priority restoration][priority-continue]. |
| Clear/pass | Clear marks the entry unused and sets its pass count; it is not necessarily immediate unlink/free. Leading/over-age markers are dequeued. A middle marker is marked for removal until handling can safely remove it. [Clear][marker-api]; [pass][marker-pass]; [dequeue][marker-dequeue]. |
| Pool full | Reuse chooses an eligible older marker already in inner, preferably unused. If none is eligible, allocation can fail and return `undefined`. Optimization must degrade without changing eligible messages. [Reuse][marker-reuse]. |
| Internal synchronous request | `erts_msgq_set_save_end` either saves current inner end or inserts a temporary marker through middle. Its caller must immediately receive the newly created private reference; generic user code cannot safely take over that cursor state. [Save-end][marker-end]; [timer synchronous wait][timer-access]. |

The inspected integer marker ID can grow into a bignum. Existing receive tests include the marker-reserve heap-pointer regression and bad-timeout cursor/marker reset. The full compiler/control-flow product, marker pool pressure, all priority modes, exceptions and every JIT remain separate qualification work. [Existing regressions][marker-regression].

## 6. Three timer families must not be conflated

| Family | State and observable consequence |
|---|---|
| Receive/process timer | Stored in `Process.common.timer`; expiry changes control flow to the receive timeout continuation. No application timeout message is enqueued. |
| BIF message timer | `start_timer` sends `{timeout, Ref, Msg}`; `send_after` sends `Msg`. A scheduler timer service owns the timed entry; a PID recipient tracks it for death/suspension. |
| Runtime callback/port timers | Internal callback or port task scheduling. They are not equivalent to a user's receive timer or a process-message timer. Their full callback/port lifecycle is outside this report. |

### Receive timeout: cancellation and expiry are separate from mailbox arrival

The wait helper guards arming with `F_INSLPQUEUE | F_TIMO`; unmatched wakeups do not restart the duration. Zero follows the scan and takes the timeout continuation directly; infinity installs no finite deadline. Initial scan and expression evaluation were distinguished in report15. [Wait helper][receive-wait].

`proc_timeout_common` changes **that exact timer pointer** to `TIMEDOUT` using compare-exchange, then synchronizes receive locks and schedules an inactive non-exiting process. Selection calls `erts_cancel_proc_timer`, which exchanges the pointer to NONE and explicitly handles an already-TIMEDOUT value. Before returning execution to a process, the scheduler redirects a non-exiting process with timeout recorded to its saved timeout continuation. [Receive expiry][receive-expiry]; [cancellation][receive-cancel]; [continuation][receive-continuation].

Therefore callback success alone is not a complete specification of which receive body runs. Cancellation can invalidate the callback's pointer; an already-recorded timeout can be cleared on selection; a scheduler can enter the timeout continuation before further matching. The exact permitted traces depend on these transitions, not application send timestamps. This audit narrows the transition graph but still has not exhausted all active/migrating/waiting/exit interleavings.

### BIF timer setup and lifetime

`parse_timeout_pos` uses monotonic time, with nonnegative relative durations or absolute external monotonic timestamps converted to the internal epoch/range. Public BIF timers are not restricted to the receive timeout's 32-bit millisecond range: their absolute deadline must fit the runtime's monotonic interval. Option parsing accepts booleans for applicable `abs`, `async`, `info`; defaults are false, false, true respectively, and inapplicable/invalid options fail. [Time parsing][time-parse]; [option parser][timer-options]; [timer contract][timer-doc].

`setup_bif_timer` accepts only an internal local PID or atom name, allocates a reference associated with the scheduler service, wraps `start_timer` content, and creates wheel/high-level storage. An alias reference is not a BIF timer destination. With PID destination it locates/retains the **recipient**, inserts the timer in the recipient tree, or cancels/disposes immediately for an absent recipient while still returning the reference. A timer created while that recipient is paused follows the paused path. [Setup][timer-setup].

This means “timer owner” must distinguish scheduler service, caller and recipient. From the traced attachment/cleanup paths, **creator death alone does not cancel a timer aimed at another live process**. PID-recipient death does; the exiting process walks and destroys its BIF-timer tree through a yielding traversal. Atom-name timers have no fixed recipient process to own that cancellation: name lookup happens at expiry, so unregister/re-register can change the destination; an absent name silently drops the message. [Recipient contract][timer-doc]; [expiry][bif-expiry]; [exit cleanup][timer-exit].

### Expiry versus cancel/read

`bif_timer_timeout` competes for ACTIVE→TIMED_OUT. `cancel_bif_timer` competes for ACTIVE→CANCELED. Only the winning active-state operation takes the corresponding delivery/disposal path. Expiry transfers the message fragment into an ordinary queued message with internal source `clock_service`, then removes tracking where safe; exiting recipients retain responsibility for their destruction walk. Cancellation does not scan the application mailbox to retract a previously queued timeout. [Expiry][bif-expiry]; [cancel/access][bif-cancel].

An integer read result reports remaining time; an integer cancellation result reports a canceled active timer, including zero remaining time. `false` conflates unknown reference, previously canceled timer and expiry; it does **not** say whether an expired message is already present, still in transit or already consumed. Non-reference inputs error; a syntactically valid nonlocal/unrecognized reference can produce the no-timer result. [Public cancel/read][cancel-doc], [read contract][read-doc]; [reference validation][timer-access].

Access branches matter for scheduling and mailbox state:

1. If the owning scheduler is current, look up its timer tree and act immediately.
2. Otherwise, a caller that is itself the PID recipient can find its timer under BTM and access directly.
3. Otherwise schedule auxiliary work on the owning scheduler. Async returns `ok`; optional `{cancel_timer,Ref,Result}` or `{read_timer,Ref,Result}` arrives later. A synchronous call creates a private reply reference, moves the receive boundary to the end, and traps awaiting that reply.
4. An async caller dying before the reply does not stop the requested cancellation from being performed; the response lookup can simply find no caller. Remote-scheduler cancellation queues physical cleanup back to the owning service, so logical cancellation and freeing are not the same event. [Access paths][timer-access]; [local/recipient shortcut][timer-access-local]; [reply construction][timer-replies]; [canceled queue][timer-cancel-queue].

High-level same-deadline expirations yield across batches; recipient-exit cancellation yields too. Cancellation-queue work has an operation limit and thread-progress requirements. None of these establishes a hard latency bound; port/runtime callback work and all timer-wheel internals were not audited. [Same-time service][timer-service]; [exit cleanup][timer-exit].

## 7. Suspension pauses timers, with a newly identified reference question

The pinned public contract says receive timers pause with suspension since OTP28, and PID-targeted BIF timers since OTP29; registered-name timers are unaffected. This is a debugger-style operation, separate from ordinary preemptive scheduling. A scheduler switching away from a runnable process does **not** grant it the same suspended-timer semantics. [Suspension contract][suspend-doc].

Receive pause captures remaining milliseconds, cancels the active timer, and stores a paused object/count. Nested pauses increment the count; final resume re-arms the remaining duration. None/TIMEDOUT pointers do not become fresh paused deadlines. [Receive pause snapshot][pause-proc-create]; [pause/resume][pause-proc].

BIF pause creates a recipient paused-list/count, copies each timer's stored message and remaining duration, then cancels its old active timer. A new PID-targeted timer created during suspension joins that paused list. Last resume rebuilds timers relative to current monotonic time. Destruction frees paused payload fragments. The pause traversal explicitly declines a yielding walk because it is debugger-only; do not import that responsiveness concession into Bream's ordinary runtime work. [Paused message copy][pause-bif-create]; [pause traversal][pause-bif]; [resume/destruction][resume-bif].

**Unexecuted source-predicted edge:** paused BIF entries store the message and remaining time but do not preserve the original timer lookup reference. Resume allocates a fresh internal reference for the rebuilt entry while preserving the stored message, which for `start_timer` still contains the old visible reference. `read_timer`/`cancel_timer` look in active scheduler/recipient timer trees, not the paused list. This predicts that using the original reference during or after pause can report `false` even though a timeout carrying that reference can later be delivered. Do not call this a confirmed upstream defect or silently copy it as an intended API guarantee. [Snapshot fields][pause-bif-create]; [new resume reference][resume-bif]; [lookup paths][timer-access-local].

Required exact-version fixture: create a long `start_timer` and `send_after` for another PID; record refs; suspend synchronously; read/cancel by original refs while paused; resume; repeat before their remaining durations elapse; observe deliveries and compare the tuple's ref. Repeat with creation during suspension, multiple suspenders, recipient death and expiration racing the pause snapshot. Sampled upstream pause tests check delivery timing/order and use `_TimerRef` variables, not read/cancel identity continuity. [Pause test][pause-test].

## 8. Provenance and tests: observations are not authentication

Local alternate-send metadata separates internal `from`, alias, token and message values; receiver conversion restores the message's internal sender field. Timer messages/replies use the atom `clock_service` internally. Ordinary receive yields the term, not an authenticated transport envelope. An application can construct a term shaped like a timeout or Down; priority status is likewise not exposed as a reliable tag in the consumed payload. Bream's accepted typed envelope still needs a deliberate observable mapping, especially for explicit flush and application-created lookalikes. Do not turn internal metadata into a stronger public security or global-order promise. [Alternate metadata][alias-create]; [timer replies][timer-replies]; [priority observability][priority-doc].

Selected upstream evidence read:

| Fixture | What the inspected assertion covers; what it does not |
|---|---|
| `timer_bif_SUITE:read_timer_async` | Same timer read→cancel→read across scheduler access expects integer/integer/false response sequence. It does not assert universal ordering between unrelated timer services. [Test][timer-read-test]. |
| `cleanup` / `auto_cancel_yielding` | Dead PID and later recipient death clear timer tracking/memory; mass recipient death exercises yielding cleanup. No host RSS or arbitrary Rust destructor promise follows. [Cleanup tests][timer-cleanup-test]; [mass cleanup][timer-mass-test]. |
| `same_time_yielding_with_cancel_test` | Explicitly accepts timeout before cancel-false **or** cancel-false before timeout; integer cancel belongs to the non-timeout count. Useful allowed-outcome fixture, not a fixed winner. [Race test][timer-race-test]. |
| `evil_timers` assertion | Sorts observed timeout sets because delivery order is not universally guaranteed. Do not infer order merely from shared `clock_service` source metadata. [Ordering caveat][timer-order-test]. |
| Priority mixed-message/receive fixtures | Check prefix order and arriving-priority redirection during scanning/waiting; full marker product remains unexecuted. [Mixed order][priority-test]; [receive][priority-receive-test]. |
| Invalid-timeout and marker-reserve regressions | Check cursor reset and allocation pointer maintenance; not all guard/exception lowering. [Regressions][marker-regression]. |

## 9. Concrete additions to the build catalogue

These are **proposed additional feature/case IDs**, not implementation, passing tests or a silent first-release expansion. Existing MSG-01–04 remain valid for their ordinary-local scope. Add source-backed acceptance rows, then derive the Rust mapping; retain multiple legal results where events race.

| Proposed branch | Fixture and required/forbidden observation |
|---|---|
| **MSG-05.C01** Alias delivery cut | Deactivate before send, after publication/before handling, and after mailbox conversion. Drop only not-yet-received alias deliveries; never flush an older received item implicitly. |
| **MSG-05.C02** One-shot alias | Two replies plus unrelated unmatched backlog. First accepted reply deactivates alias independent of application selection; later alias reply drops. Test competing senders as a permitted set. |
| **MSG-05.C03** Alias state/priority | Never-alias reference, pending spawn alias, ordinary alias, priority alias × ordinary/priority send. No active-state side channel through ordinary successful send. |
| **MSG-05.C04** Alias ownership/kill | Stop recipient at copy, publication, drop and conversion; exactly one owner/release for payload and immutable-buffer refs. Share relationship cleanup fixture with LIFE. |
| **MSG-06.C01** Priority order | Mixed application/Exit/Down prefix order preserves each class; same-sender priority can overtake ordinary content, without reordering internal signals. |
| **MSG-06.C02** Priority during receive | Insert while cursor is ordinary, priority, exhausted or waiting. Newly eligible priority message must be seen; continuation must resume without losing ordinary content. |
| **MSG-06.C03** Last priority capability | Remove last alias/link/monitor with prefix nonempty; then re-enable. Already accepted messages remain, and one-shot first reply retains priority. |
| **MSG-06.C04** Reference search cost | Large nested priority payload containing/no matching marker reference. Verify both eligible-message equivalence and scheduler/kill responsiveness; no unbudgeted walker. |
| **MSG-07.C01** Marker reserve/bind/clear | Empty/nonempty queues, valid/invalid bind, absent/retired marker, pool pressure and big temporary IDs. Optimization may change inspections, never selected messages. |
| **MSG-07.C02** Deferred marker + priority | Marker in middle, fresh reply in priority prefix, nested use and timeout/error reset. Compare optimized/unoptimized legal traces. |
| **MSG-04.C08** Buffer transitions | Pending monitor followed by direct/buffered messages; install/flush/uninstall races; first nonempty publication versus sleep. No sender reordering or lost wakeup. |
| **MSG-01.C09** Send option boundary | Valid/invalid/improper option lists; local dead/live PID with nosuspend/noconnect; remote/port only in later transport profile. No bounded-mailbox interpretation. |
| **TIME-01.C01** Timer input/payload | `start_timer` versus `send_after`, zero/large/absolute-past/range-overflow, bad options and alias/nonlocal destinations. Correct result/error and wrapper; no receive-range reuse. |
| **TIME-01.C02** Creator/recipient/name | Creator dies, PID recipient dies, name unregisters/rebinds before expiry. Preserve recipient cancellation versus late name lookup; no creator-owned automatic cancellation. |
| **TIME-01.C03** Cancel/read mode matrix | Current/other scheduler × caller-is-recipient/other × sync/async × info. Check response shape, suppressed notifications and caught errors; no false-as-mailbox-empty claim. |
| **TIME-01.C04** Expiry/cancel race | Enumerate ACTIVE CAS outcomes; permit both timeout/cancel-false arrival orders. Integer cancel cannot also produce that active timer's timeout; no implicit mailbox retraction. |
| **TIME-01.C05** Timer cleanup progression | Many same-deadline timers and recipient-death cancellation across services. Account for payload release and peer progress separately from logical cancellation. |
| **TIME-01.C06** Suspended timers | Receive/PID BIF/name BIF × nested pause/new timer/death. Reference fixtures establish remaining-time behavior; normal scheduler preemption must not pause deadlines. |
| **TIME-01.C07** Paused reference continuity | Execute the original-ref fixture above and reconcile source with intended contract before choosing a Rust result. Keep prediction explicitly unverified. |
| **MSG-03.C10** Receive expiry refinement | Callback pointer CAS, already-TIMEDOUT cancellation, scheduler continuation, publication and kill. No timestamp winner, duplicate completion or stale-generation timeout. |

Remaining research is concrete: exact-version execution of the two timer race families, paused-reference continuity, systematic buffer lifecycle memory-order exploration, all priority/marker state products, dynamic receive guard/timeout lowering, and cross-runtime typed-notification equivalence. Distribution encoding, ports, trace injection, dirty signal-handling ownership and timer-wheel internals remain separately scoped. This deeper inspection expands the branch inventory; it does not support “all process behavior is now figured out.”

Local trace entry points: [alias delivery](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6009), [priority insertion](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:9300), [timer access](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_hl_timer.c:2237), [paused timer rebuild](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_hl_timer.c:2949).

[queues]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.h#L337-L423
[send-dispatch]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L2458-L2488
[ordinary-drop]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.c#L370-L459
[pending]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L1076-L1167
[prepend]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.c#L462-L479
[signal-publish]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L1188-L1251
[enqueue-flush]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L896-L911
[buffer-select]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L10260-L10306
[buffer-race]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L10308-L10365
[buffer-flush]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L10494-L10561
[fetch]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L1332-L1433
[fetch-repair]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L1438-L1495
[wait-handoff]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7771-L7798
[buffer-wakeup]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L10381-L10419
[signal-limit]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6968-L6971
[alias-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L68-L111
[alias-create]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L2077-L2289
[alias-handle]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6009-L6135
[alias-prio-release]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6210-L6219
[send-options]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L2697-L2798
[send-options-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L8723-L8778
[nosuspend-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L11772-L11855
[old-incarnation]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L2489-L2505
[priority-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L361-L398
[priority-enable]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L415-L456
[priority-insert]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L9300-L9487
[priority-continue]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4484-L4586
[priority-walk]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L9139-L9297
[priority-lifetime]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L9502-L9562
[priority-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/signal_SUITE.erl#L1830-L1915
[priority-receive-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/receive_SUITE.erl#L798-L851
[priority-marker-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/signal_SUITE.erl#L2255-L2326
[compiler-marker]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/compiler/src/beam_ssa_recv.erl#L27-L80
[marker-api]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.h#L2203-L2299
[marker-use]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.h#L2123-L2200
[marker-pass]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4431-L4478
[marker-dequeue]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4046-L4077
[marker-reuse]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4148-L4282
[marker-end]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.h#L2397-L2421
[marker-regression]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/receive_SUITE.erl#L694-L787
[receive-wait]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/beam_jit_common.cpp#L1188-L1255
[receive-expiry]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L691-L706
[receive-cancel]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2831-L2845
[receive-continuation]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10451-L10464
[time-parse]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2493-L2558
[timer-options]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2351-L2398
[timer-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L6215-L6245
[timer-setup]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1803-L1870
[bif-expiry]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1145-L1206
[timer-exit]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2401-L2489
[bif-cancel]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1873-L1971
[cancel-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L1491-L1536
[read-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L5798-L5833
[timer-access]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2208-L2338
[timer-access-local]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2077-L2205
[timer-replies]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1991-L2074
[timer-cancel-queue]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1700-L1790
[timer-service]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1394-L1447
[suspend-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L6275-L6291
[pause-proc-create]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1029-L1056
[pause-proc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2848-L2945
[pause-bif-create]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L994-L1022
[pause-bif]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2873-L2915
[resume-bif]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2948-L3027
[pause-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L1851-L1897
[timer-read-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/timer_bif_SUITE.erl#L294-L323
[timer-cleanup-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/timer_bif_SUITE.erl#L325-L360
[timer-mass-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/timer_bif_SUITE.erl#L712-L730
[timer-race-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/timer_bif_SUITE.erl#L682-L701
[timer-order-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/timer_bif_SUITE.erl#L476-L482

Plan integration: these case IDs are recorded in the [expanded build catalogue](../build-plan/process-runtime-expansion.md); report 19 also extends the original [messaging tables](../build-plan/messaging.md). All remain planned, with conditional applicability preserved.
