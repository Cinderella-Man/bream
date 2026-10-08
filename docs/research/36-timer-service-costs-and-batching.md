# Timer-service costs, grouping and batching

Inspected **2026-10-07**, Erlang/OTP **OTP-29.1.1**, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. This is read-only source research: **no timer benchmark, fixture or upstream test was executed for this report**. Performance ideas below are proposals, not measured improvements. [Report 19](19-process-signals-timers-deep-dive.md) covers public timer semantics; [report 31](31-receive-parking-wakeup-protocol.md) covers receive parking. This report follows the additional indexing, batching and reclamation branches that those reports left sampled.

## 1. Two thresholds, two indexes, multiple costs

A BIF timer's **5,000 ms “short time” threshold is an allocation preference**, not the boundary between the timing wheel and high-level timer tree. Parsing computes `short_time` and a deadline; public BIFs independently choose the wheel when the duration is strictly below `ERTS_TIMER_WHEEL_MSEC`. Short BIF timers try scheduler-preferred preallocation and fall back to allocation. Ordinary process/port wheel timers use a separate allocator. Conflating these thresholds would produce a misleading benchmark. [Threshold constant][short-time]; [parsing and routing][parse]; [allocation][alloc].

The wheel uses “soon” and “later” slot arrays. The configured sizes, clock resolution and derived horizon matter: with 1 ms ticks the normal 14-bit arrays give a roughly **37.3-hour** routing horizon; 12-bit SMALL_MEMORY gives about **2.3 hours**, and 10-bit wheel-debug arrays about **8.7 minutes**. These are calculations from this revision's macros, not universal API limits. Each scheduler's wheel has allocated arrays; empty runtimes therefore have a different space cost from “bytes per live timer.” [Configuration][config]; [horizon][horizon]; [wheel allocation][wheel-alloc].

A BIF timer also belongs to an owner-service lookup tree keyed by reference, and PID-targeted timers can belong to the recipient's timer tree. A cheap deadline insertion does not eliminate those indexes or payload creation. Non-immediate timer messages are sized and copied into a message buffer during creation. [Payload and owner index][btm-init].

| Branch | Work and retained state | Performance consequence |
|---|---|---|
| Deadline at or before wheel position | Append to the special at-once slot; stored position is clamped to current wheel position. | “Zero delay” still entails service work; it is not an inline user callback at the BIF call site. [Wheel insertion][wheel-set]. |
| Deadline strictly inside soon range | Insert into a circular slot list. | Direct cancellation has a stored slot/list location; no mailbox-like search. [Insertion][wheel-set]; [removal][wheel-remove]. |
| Later wheel slot | Slot represents a range; servicing can retain a far-future timer, promote it to soon, or expire it when already overdue. | Catch-up work includes moves as well as deliveries. [Later sweep][later]. |
| Beyond BIF wheel-routing horizon | Insert into high-level time tree, with one service wheel timer for its next deadline. | A new earliest deadline cancels/rearms that service timer. [HL insertion][hl-create]. |
| Exact duplicate HL deadline | Existing tree representative owns a circular same-time list. | Fewer time-tree nodes does not mean fewer timer identities, payloads, cancellations or deliveries. [Grouping][same-time]; [insertion][hl-create]. |

Sparse wheels avoid a naive scan of every slot: range counters cover 512-slot blocks, soon-range search can retain an exact minimum, and later-range search can arrange an earlier maintenance wakeup. The next exposed timeout is capped at a week for polling calculations; that cap is not a restriction on accepted user deadlines. A wakeup used to move/check timers must not be mistaken for permission to deliver them early. [Range counters][range-count]; [next timeout search][next-timeout].

Clock reading is also on the path: timeout parsing calls `erts_get_monotonic_time`, whose configured provider updates scheduler-local monotonic-time bookkeeping. Relative conversion includes rounding arithmetic. Caching time more aggressively is therefore a semantic change candidate requiring no-early-expiry and range tests, not an automatically safe removal of a syscall. The provider is configurable; this review did not establish one clock-call cost across platforms. [Conversion][parse]; [clock dispatch][clock]; [bookkeeping][clock-update].

## 2. Equal deadlines share an index, not a lifecycle

`same_time_list_insert/delete` maintain circular links. Deleting a non-representative removes that list member. Deleting the representative with surviving members replaces it in the tree and updates `next_timeout`; deleting the last timer at the minimum cancels the service timer and rearms it for the next minimum. These are distinct cancellation branches with different pointer and wheel work. [List operations][same-time]; [HL deletion][hl-delete].

At expiry the service removes the representative from the time tree before walking its companions. The list walker can preserve remaining work in `yield.root`; the service then rearms at that group's original deadline. The configured limit is **100 list members** per traversal, or **5** under HL debug. On the completing invocation it also processes the representative, so this is not a strict maximum of 100 total timer callbacks per invocation. A canceled timer whose time-tree marker is already `NOT_IN_TABLE` does not undergo another tree removal: outstanding traversal ownership still matters. [Limits][limits]; [destructive traversal][list-yield]; [expiry/rearm][hl-service]; [already-detached deletion][hl-delete].

Grouping preserves exact deadline equality. It does **not** establish public FIFO order among all equal-deadline timers, merge timeout messages, or round nearby deadlines into a common bucket. Any Bream experiment must retain independently cancelable references and the established cancel-versus-expiry outcome set; group membership is internal indexing. [Representation and list order][same-time]; [cancel winner][cancel].

## 3. Local helper limits are not a scheduler latency bound

The wheel charges a nominal budget: ordinary builds start with 10,000 units, with charges of 1 per slot, 5 per move and 100 per timeout callback. It invokes callbacks synchronously, after marking the wheel entry inactive. A wheel callback can itself run the HL list traversal or deliver a timer message; its actual cost is not necessarily one constant-sized action. [Wheel costs][wheel-cost]; [callback dispatch][callback]; [HL dispatch][hl-dispatch].

When sweeping a slot, the wheel detaches its list into a sentinel. Cancellation can still remove an entry from this detached list, and a yield records which slot/sentinel remains in progress. Soon and later sweeps also inspect future collisions and move timers. These branches make a burst benchmark with only empty callbacks insufficient to characterize cancellation, catch-up and delivery fairness. [Removal from detached lists][wheel-remove]; [soon sweep/yield][soon]; [later sweep/yield][later].

**Source-predicted edge requiring execution:** an isolated, repeatedly rearmed HL service timer appears able to continue through many list batches within one outer `erts_bump_timers` call. The precise chain is:

1. HL expiry saves remaining group work and rearms its service timer at the due deadline. [Service continuation][hl-service].
2. A deadline at/before wheel position enters the at-once slot. [Insertion][wheel-set].
3. The at-once loop invokes the callback and subtracts its cost. It then breaks when its detached sentinel is empty **before** checking whether the budget is exhausted. If wheel position has reached the bump target and another at-once timer was installed, it loops again. [Critical branch order][at-once].

Thus “the HL walker yielded” does not by itself mean another process received a turn. This is a bounded source hypothesis about this composition, **not a reproduced stall, an upstream bug finding, or a proof that every burst drains without a scheduler return**. Instrument outer bump entries/exits, each rearm and runnable-peer dispatch before claiming a fairness bound. Normal scheduler code calls the bump outside the run-queue lock, but that unlock alone is not a switch to another process on the same worker. [Scheduler integration][sched-bump].

Selected upstream test names need similar care. `same_time_yielding` creates absolute deadlines roughly 20 seconds ahead, and its cancel variant roughly 6 seconds ahead. Under the common configurations calculated above, both remain wheel timers; their names do not establish coverage of the current HL same-deadline traversal. They check eventual counts/cancel results and elapsed-time conditions, not the complete peer-dispatch trace proposed here. In contrast, `auto_cancel_yielding` uses durations beyond `1 bsl 28` ms, reaching the HL branch in those configurations. These are source-reviewed tests, not test results from this investigation. [Same-time test][test-same]; [cancel test][test-cancel]; [death cancellation][test-death].

## 4. Cancellation has a logical winner and several later costs

The ACTIVE→CANCELED compare-and-exchange settles cancellation ownership. The winning path can free the payload before returning and remove recipient-side bookkeeping. For a timer owned by another scheduler, physical owner-index/wheel cleanup can be queued back. A local owner accesses its index directly; a foreign owner can sometimes be bypassed for the caller's own recipient timer, while other accesses allocate an auxiliary request and arrange the selected reply/wait behavior. The public matrix is already in report19; the new measurement distinction is **CAS/payload release, request/reply routing, and owner cleanup are separate phases**. [Winner and payload][cancel]; [owner cleanup selection][access]; [recipient shortcut][self-access]; [remote request][remote-access].

| Transition / owner | Work that may finish here | Work that can remain |
|---|---|---|
| Canceling process wins ACTIVE→CANCELED | Prevent expiry from winning that timer; release message buffer; detach recipient index when appropriate. | Foreign owner's deadline/reference indexes, timer allocation reclamation. [Cancel][cancel]; [access][access]. |
| Producer queues timer to owner | Atomic queue linking publishes cancellation work. A tail-advancing enqueue triggers notification; not every enqueue does. | The owner cannot immediately free every reachable queue node while producers can still hold links. [Producer algorithm][cq-enqueue]; [notification][cq-notify]. |
| Owner advances eligible queue boundary | Thread-progress observation plus explicit memory barrier permits `unref_end` to advance. | Newer nodes wait for a later cohort; eligibility is distinct from backlog size. [Boundary][cq-progress]. |
| Owner drains eligible entries | Removes service index and cancels/removes deadline entry, up to 100 entries per limited pass. | More-work/progress flags can schedule continuation; a nominal entry count does not bound each entry's cost. [Cleanup][owner-cleanup]; [drain][cq-drain]. |
| Final timer reference is released | Releases associated retained process record where applicable. | Allocation destruction is scheduled through thread-progress-later cleanup. [Wheel final release][tw-release]; [HL final release][hl-release]. |

The scheduler's canceled-timer auxiliary handler clears its work flag before draining, rearms for additional work, and requests progress-related wakeup when retirement is not yet eligible. This is why a batching treatment needs a progress/flush policy for a **small final batch**, not only a throughput threshold. The queue's producer CAS branches can insert into an existing chain after contention; this review does not claim global FIFO cancellation order. It traced the named publication/drain edges, not a full weak-memory proof of this queue. [Auxiliary work][aux]; [producer branches][cq-enqueue].

Process death walks recipient timers with a resumable tree iterator. Each callback charges 80 reductions even though it can free a payload, touch multiple indexes, or queue work remotely. Likewise, a 100-entry canceled-queue pass is count accounting. Neither constant establishes a wall-time bound across payload shapes, allocator behavior or retention backlog. Also, opportunistic cancellation checking at timer creation is compiled off by `ERTS_TMR_CHECK_CANCEL_ON_CREATE=0` in this revision; do not credit that inactive helper with reducing backlog. [Exit cancellation][exit-cancel]; [disabled create-check flag][create-check-flag]; [guarded helper][create-check].

## 5. Proposed additional branch cases

These **nine cases are planned and unexecuted**, extending existing TIME-01.C01–C07. They refine structural branches rather than replace the existing API/race matrix. Test setup can use instrumented clocks or internal hooks; advancing a clock in a harness must not be presented as elapsed real time.

| ID | Trigger and required observation | Forbidden outcome / open oracle |
|---|---|---|
| **TIME-01.C08** | Cross both allocation-preference and wheel/HL routing thresholds at −1/exact/+1; repeat relative/absolute forms and selected wheel configurations. Record actual backend and pool fallback. | Treating 5 seconds as the tree boundary; changing accepted deadlines or rounding while switching representation. Sources: [parse][parse], [allocation][alloc]. |
| **TIME-01.C09** | Hit at-once, last soon position, exact soon boundary and later wrap; delay service so a later timer is retained, promoted or already overdue. | Early expiration, duplicate/missing callback, or infinite revisit of a future collision. [Wheel set][wheel-set]; [later][later]. |
| **TIME-01.C10** | Exhaust wheel work with a nonempty detached sentinel, then cancel its first/middle/last remaining member before resume. | Stale slot pointer, double unlink/free, canceled entry executing, or loss of other entries. [Removal][wheel-remove]; [soon][soon]. |
| **TIME-01.C11** | Cancel HL list member, representative with survivors, singleton minimum, and representative after expiry traversal detached the group. | Lost replacement minimum, duplicate service timer, resurrection, or premature release of traversal-owned state. [HL deletion][hl-delete]; [service][hl-service]. |
| **TIME-01.C12** | Large exact-deadline HL group with one runnable peer; trace repeated at-once rearm at/after budget exhaustion. Compare singleton and multi-entry at-once lists. | Declaring a scheduler bound from the inner limit alone. Actual return/dispatch trace remains an exact-version execution question. [Critical branch][at-once]; [service][hl-service]. |
| **TIME-01.C13** | Sparse later-only wheel, delete cached minimum, insert earlier deadline, and exercise a maintenance/pre-poll wakeup with a distant surviving timer. | Sleeping past a newly earlier deadline, early delivery at maintenance wakeup, or confusing the week poll cap with a user-timeout limit. [Search][next-timeout]; [set][wheel-set]. |
| **TIME-01.C14** | Contend remote-cancel producers on tail and alternate insertion; stop after a small final batch near a thread-progress boundary. Trace publish, eligibility, notify, drain and empty-state transitions. | Stranded eligible work without continuation, freeing a producer-accessible node, or lost/double owner cleanup. No global concurrent FIFO requirement. [Queue][cq-enqueue]; [progress][cq-progress]; [aux][aux]. |
| **TIME-01.C15** | Equal timer counts with immediate, copied compound and off-heap-resource payloads; expire/cancel/kill recipient on local and remote-owner paths. Record time inside each nominally charged callback. | Equating 80 reductions/100 entries with a time cap; leaking/double-releasing payload on the winning transition. [Payload][btm-init]; [exit][exit-cancel]. |
| **TIME-01.C16** | Separate successful logical cancel, owner index removal, final process-record release and timer allocation reuse; introduce a delayed progress participant and allocation pressure. | Reuse before the timer's retirement protocol permits it; treating a response, empty recipient tree or retained allocator arena as proof of completed reclamation. [Owner cleanup][owner-cleanup]; [final wheel release][tw-release]; [final HL release][hl-release]. |

## 6. Performance experiments to put on the task list

All seven are **unmeasured hypotheses**. Start from a correct baseline and preserve the public timeout/read/cancel outcome set, independent identities, safe cleanup and no-early-expiry semantics. Measure peer latency and retained memory alongside throughput; reject treatments that merely defer cost beyond the measurement interval.

| Local task | Baseline → treatment | Measurements and semantic constraint |
|---|---|---|
| **TMR-P01: distribution-aware index** | One straightforward deadline index → wheel/tree hybrid with several tested horizons and optional lazy wheel allocation. | Setup/cancel/expiry cost, sparse scans, idle bytes per worker, total bytes per live timer. Cover C08/C09/C13; record configuration instead of copying BEAM constants. [Index evidence][wheel-set]; [allocation evidence][wheel-alloc]. |
| **TMR-P02: exact-deadline grouping** | One ordered node per timer → representative plus individually owned same-deadline members. | Index operations, cancellation branch mix, dense-burst time and memory. Cover C11; no rounding or public equal-deadline ordering promise. [Grouping][same-time]; [replacement][hl-delete]. |
| **TMR-P03: budget survives rearm** | Helper-local operation limit → shared service-work budget observed across nested traversal and at-once continuation. | Callbacks per outer bump, peer dispatch p50/p99/max, timer lateness, throughput. First execute C12 on the pinned reference; no claim the replacement improves speed or gives a real-time guarantee. [Composition][at-once]; [HL limit][hl-service]. |
| **TMR-P04: ownership locality** | One shared timer service → worker-owned timer state with explicit routed operations and eligible recipient-local shortcut. | Local/remote fraction, index lookups, request allocation, cancel response tails and migration workloads. Preserve cancellation's winning transition and timer identity; do not implement a shortcut without retained-state ownership. [Access matrix][access]; [request path][remote-access]. |
| **TMR-P05: cancellation notification batching** | Per-operation notification/cleanup scheduling → bounded batches with explicit progress and final-batch flushing. | Atomic retries, wakeups, owner backlog, retained bytes and reclamation delay. Cover C14/C16 and stalled participants; throughput cannot excuse indefinitely retained work. [Queue protocol][cq-progress]; [auxiliary work][aux]. |
| **TMR-P06: allocator policy** | Uniform timer allocation → measured size/lifetime-specific worker pools or reusable slots. | Cold/warm allocation, pool exhaustion/fallback, high-water retention, stale-reference/reuse checks and shutdown cleanup. Keep allocation policy independent from deadline routing; use C08/C16. [Pool selection][alloc]; [retirement][tw-release]. |
| **TMR-P07: payload-sensitive service accounting** | Fixed charge per expiration/cancel → accounting and resumable runtime work informed by admitted payload/resource shape. | Actual callback duration, copied bytes, resource-release work, peer latency and total cost through final cleanup. Use C15; immutable shared bytes may reduce copying but do not justify arbitrary destructors or premature release. [Payload][btm-init]; [fixed exit charge][exit-cancel]. |

## Remaining limits

Traced: wheel placement/promotion/cancellation, HL grouping and representative changes, service rearm, principal BIF cancellation routes, canceled-queue eligibility/draining, selected scheduler handoff and final timer release. Sampled: allocation macros, clock-provider dispatch and upstream coverage. **Not established:** a complete weak-memory proof of the cancellation queue, every internal process/port/callback timer caller, every suspended-scheduler migration path, each clock provider/time-warp configuration, allocator worst cases, or a measured scheduler latency bound. Earlier suspended-timer/reference findings remain in report19/report29 and are not re-litigated here. No production Bream timer representation or tuning constant is selected by this report.

Local navigation: [timer service](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_hl_timer.c:1395), [at-once wheel loop](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/time.c:842), [canceled-queue progress](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_hl_timer.c:1660), [upstream same-time tests](/home/bat/projects/bream/upstream/otp/erts/emulator/test/timer_bif_SUITE.erl:547).

[parse]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2494-L2641
[alloc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L875-L974
[config]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_time.h#L41-L72
[horizon]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_time.h#L147-L156
[wheel-alloc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L1169-L1216
[btm-init]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L722-L755
[wheel-set]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L1256-L1309
[wheel-remove]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L688-L749
[later]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L1030-L1167
[hl-create]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1218-L1341
[same-time]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L392-L441
[range-count]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L255-L259
[next-timeout]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L476-L644
[clock]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_time_sup.c#L1890-L1897
[clock-update]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_time_sup.c#L264-L281
[hl-delete]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1451-L1510
[limits]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L253-L259
[list-yield]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L453-L494
[hl-service]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1395-L1447
[cancel]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1874-L1918
[wheel-cost]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L218-L225
[callback]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L771-L780
[hl-dispatch]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1358-L1385
[soon]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L913-L1026
[at-once]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/time.c#L842-L894
[sched-bump]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L9797-L9809
[access]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1922-L1971
[self-access]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2174-L2205
[remote-access]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2237-L2328
[cq-enqueue]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1561-L1609
[cq-notify]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1769-L1774
[cq-progress]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1634-L1686
[owner-cleanup]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1519-L1542
[cq-drain]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1701-L1764
[tw-release]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L802-L835
[hl-release]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1080-L1120
[aux]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L2187-L2272
[exit-cancel]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L2402-L2483
[create-check]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1134-L1142
[test-same]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/timer_bif_SUITE.erl#L547-L587
[test-cancel]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/timer_bif_SUITE.erl#L640-L710
[test-death]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/timer_bif_SUITE.erl#L712-L733

[short-time]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L93-L108
[create-check-flag]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L38-L43

The proposed cases are integrated in the [build plan](../build-plan.md). Report-local experiment names feed the separately numbered [performance backlog](../performance-plan.md); they are not additional executed tests or measured results.
