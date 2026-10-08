# Worker sleep, wake and scheduler-controller boundaries

Research date: **2026-10-08**. Reference: Erlang/OTP **OTP-29.1.1**, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`, verified against the local reference checkout. **This report is source review only: no upstream test, runtime fixture or benchmark was executed for it.** Separately executed Rust notification experiments are evidence about their stated abstraction, not execution of these ERTS branches. Every performance candidate below is a hypothesis.

[Report 35](35-scheduler-locality-and-load-balancing.md) traced balancing and selected spin policy; [report 46](46-thread-progress-and-retirement-domains.md) traced participant/quiescence protocols; [report 47](47-process-lock-ownership-and-handoff.md) distinguished execution claims from field locks. This pass connects worker enrollment, the physical sleep primitive and the online/multi-scheduling controller. [Report 31](31-receive-parking-wakeup-protocol.md) remains the separate process receive-parking protocol. Waking an OS worker neither activates a particular receive continuation nor grants its process execution claim.

The material new result is that **a worker can be forbidden to dispatch application work while still owning timers, evacuation and auxiliary work**. Conversely, a worker leaving physical sleep may remain in its idle-service loop. A Bream optimization must preserve these service obligations, not just arrange that a queue consumer eventually wakes.

## 1. Worker wait enrollment carries the queue protection

The scheduling loop checks runnable work under the run-queue lock. After an unsuccessful steal it rechecks suspension and the full multi-scheduling/dirty-work condition, then enters `scheduler_wait` while still holding that lock. `scheduler_wait` installs `SLEEPING|WAITING`, preserving `MSB_EXEC`; if it observes suspension it returns to the controller. Only after installing this state and recording that the scheduler is waiting does it unlock the run queue. The producer therefore cannot insert ordinary run-queue work through that lock between the final empty decision and wait enrollment. [Empty scheduling path][S01] [Enrollment CAS][S02] [Locked enrollment/unlock][S03]

The flags describe different dimensions:

| State | Meaning in the inspected path | Incorrect inference |
|---|---|---|
| `WAITING` | Scheduler remains in the idle-service/wait protocol. | OS thread is physically blocked, or no auxiliary work is running. |
| `SLEEPING` | This wait iteration can still proceed toward physical sleep. | A syscall has already blocked, or every wake requires a syscall. |
| `TSE_SLEEPING` / `POLL_SLEEPING` | A physical notification destination has been installed. | These destinations can never coexist. Thread-progress blocking while polling explicitly handles both. |
| `SUSPENDED` | Controller forbids ordinary scheduling until its condition is released. | This thread has no runtime-service obligations. |
| `MSB_EXEC` | Scheduler participates in serialized cross-type execution while full multi-scheduling is blocked. | Permission to ignore other controller state or a process execution claim. |

Sources: [flag definitions][S04], [idle-loop service/exit][S08], [physical wake routing][S07], [suspended loop][S15], [cross-type handoff][S19]. Names such as “active” also occur in the load balancer, thread-progress domain and controller counts; they must retain their domain rather than becoming one Bream boolean.

## 2. Reset-before-enrollment closes the early-notification window

`sched_set_sleeptype` first claims and resets the thread event, or clears the polling interrupt, **then** CAS-installs the sleep destination. Its CAS retries only while both `SLEEPING` and `WAITING` remain set. A wake that wins before destination installation changes those flags, so the worker cannot reset the event and then blindly wait. A wake that wins after installation sees the destination in the old flags and signals it. [Destination enrollment][S05]

This produces a source-derived race argument, not a complete weak-memory proof:

| Wake point | Source path that prevents an indefinite blind wait |
|---|---|
| Before ordinary queue-wait enrollment | Queue protection keeps the empty check and enrollment together; later queue inspection retains published work. |
| After enrollment, before or during event reset | Wake changes the flags. Destination CAS observes the incompatible state and declines to sleep. |
| After destination CAS, before the actual event wait | Waker signals the installed destination; the event retains its set state for the arriving waiter. |
| During event wait | Waker signals the corresponding event or polling interrupt. Interrupted waits recompute remaining timer time. |
| After a wait returns, before another iteration | Persistent service/queue state and continuation enrollment decide whether another wait is permitted; the return itself is not evidence of executable application work. |

Sources: [locked enrollment][S03], [destination CAS][S05], [wake-state CAS][S06], [wake routing][S07], [continuation enrollment][S02], [timed wait/recheck][S08]. On the inspected Linux futex backend, `event_set` atomically sets the event and issues a futex wake only if the prior state recorded a waiter. The wait checks for an already-set event, installs the waiter state with CAS and waits on the expected futex value. The reset and event memory barriers belong to that complete protocol; replacing individual C operations with similarly named Rust orderings is not a proof. [Linux event set/reset][S09] [Linux wait][S10]

**Direct queue wake and auxiliary poke are different operations.** `ssi_flags_set_wake` clears `WAITING` as well as sleep bits, retaining `SUSPENDED|MSB_EXEC`; `erts_sched_poke` clears only the sleep bits. Thus auxiliary work can wake the physical worker without ending the idle-service loop. Auxiliary flag publication coalesces already-present reason bits and pokes only when new bits are installed. The poke contains a full barrier before reading sleep state; the worker reads auxiliary work after enrollment. Both publication and wake-state ordering matter. [Direct wake][S06] [Poke][S11] [Auxiliary publication][S12]

`erts_sched_finish_poke` also has a combined `POLL_SLEEPING|TSE_SLEEPING` branch: thread-progress blocking during poll sleep needs both destinations signaled. A Rust enum allowing exactly one active wake destination would erase a source-relevant composite state unless the surrounding architecture removed that overlap with evidence. Normal queue wake deliberately runs without the queue lock for performance; the source says holding that lock is not inherently a safety/deadlock requirement. This supports measuring notification after publication/unlock rather than assuming the syscall must occur inside the queue critical section. [Both destinations][S07] [Wake lock expectation][S13]

## 3. Shared dirty queues add sleeper-list ownership

Normal queues address their assigned scheduler; dirty queues can have several sleeping workers. A dirty worker enrolls its sleep record on a locked circular list before releasing the run-queue lock. `wake_dirty_scheduler` removes a selected record under the sleeper-list lock, clears its links and only then changes its wake flags and signals it. A worker leaving the wait removes itself if it is still linked, so waker removal and self-removal must agree. If the list is empty, the source falls back to the run queue's scheduler wake path. [Dirty enrollment][S03] [Dirty extraction][S14] [Self-removal][S08] [Notify dispatch][S27]

The list is ownership bookkeeping, not a count of jobs or proof of fair dispatch. A single sticky wake token can coalesce several notifications because persistent queues retain the work. Conversely, replacing a shared dirty queue with one token addressed to an arbitrary worker requires proof that an eligible worker remains responsible through suspension. The online-decrease path explicitly wakes dirty CPU scheduler zero after suspending higher-index workers because a newly suspended worker may have been about to handle a task. [Dirty decrease safeguard][S17]

## 4. Idle and suspended workers continue runtime service

An ordinary idle normal scheduler checks auxiliary work before sleeping. While auxiliary work remains it also checks timer expiry. Otherwise it obtains the next timer deadline, decides whether expiry is already due, and chooses scheduler polling or a timed thread event. Scheduler polling itself obtains exclusive system-scheduling permission and rechecks that outstanding port I/O tasks do not make that choice inappropriate. Timed event waits recompute current time after `EINTR`; after waiting, due timers are bumped. Dirty schedulers take a different branch: no auxiliary work is allowed there and their ordinary idle timeout is infinite. [Auxiliary/timer/event loop][S08] [System-poll admission][S28]

The suspension path is more consequential. A normal scheduler other than number one evacuates its run queue before entering suspension, preserving bound-process records through the held-list mechanism from report 35. **While suspended**, it can process auxiliary work, evacuate work that arrives later, and bump its own timer wheel. It marks thread progress inactive and performs the proper prepare/finalize-wait pair around physical waiting. On resumption it reacquires the run-queue lock and reschedules held bound records. [Entry/evacuation][S15] [Suspended service][S16] [Resume boundary][S18]

Therefore “take a worker offline” in this reference is not “terminate the OS thread,” “migrate every timer” or “finish every retirement action.” A future Rust worker pool may transfer service ownership and truly park/remove surplus threads, but that is a new implementation with transfer, publication and shutdown obligations. Bream's process stop-before-`Down` requirement remains independent: fewer online workers is not proof that a selected process has stopped.

## 5. Online changes and multi-scheduling blocks compose

The controller distinguishes requested online counts, acknowledged current-online counts, active counts and outstanding change flags. A normal online-change caller is suspended **before** requesting the change to prevent resume-before-suspend races. Concurrent normal requests queue; when acknowledgment finishes, the current caller resumes and the next queued caller obtains the change right. The source encodes a right in transit separately from an active changer. If a queued process exits while that right is in transit to it, `abort_sched_onln_chng_waitq` removes it and passes the right to the next waiter or clears it. Exit invokes this cleanup. [Request enrollment/serialization][S20] [Acknowledgment/handoff][S21] [Abort-in-transit][S22] [Exit integration][S23]

Changing online counts during a multi-scheduling block does not resume workers that remain blocked. Increase/decrease branches poke such workers so they acknowledge controller state; the ordinary no-block branch resumes/suspends run queues instead. Dirty-only adjustments can return without waiting for full dirty-worker acknowledgment. The upstream basic test explicitly delays before observing resulting dirty counts because the operation is not fully synchronous. Do not fabricate a single synchronous “all worker state settled” acknowledgment for every variant. [Blocked versus ordinary changes][S17] [Completion branches][S26] [Acknowledged-state observation][S29] [Upstream asynchronous qualification][T01]

Normal-only and full multi-scheduling blocks have separate blocker records. Repeated blocks add owned records; unblocking removes applicable records, and only the absence of blockers/pending requests releases that mode. Normal run queues resume only when neither block remains. Releasing full block can still resume dirty schedulers while the normal-only block remains. Exit removes all of the exiting process's owned records via the same controller. [Block publication][S24] [Unblock composition][S25] [Exit integration][S23]

Full block also does **not** eliminate dirty computation. Once other workers acknowledge suspension, the controller marks scheduler one for serialized execution. `msb_scheduler_type_switch` chooses between normal, dirty CPU and dirty I/O according to available queue priorities, alternating dirty types when eligible priorities tie. After dirty execution it returns to normal execution so I/O gets checked. When normal work is empty but dirty work remains, it sets the normal queue's `MISC_OP` flag to keep the normal scheduler from sleeping before handing execution back. It changes the old/new scheduler permission flags before waking the new owner. [Establish serialized mode][S21] [Priority choice and return to normal][S19]

**Bream consequence:** thread identity, logical worker permission and process execution ownership are separate axes. Rust can make the transitions explicit without copying every ERTS flag, but must not use an OS-thread notification token as permission to dispatch. The initial single-worker profile does not need to expose online-count or multi-scheduling APIs merely because these reference branches exist.

## 6. Rust simplifications worth testing

A queue plus `Mutex`/`Condvar` can express the simplest baseline: test all wake-relevant predicates under the publication mutex, then atomically release that mutex and wait. Wakeups may be spurious; the predicate must be checked again. Condvar notifications are not buffered. Therefore a check, explicit unlock, then unconditional wait is a deliberately broken comparison, not the baseline. [Rust Condvar contract][R01]

For a dedicated runtime-owned worker, a persistent queue plus `Thread::unpark`/`park` may remove a separate event object and the explicit event-reset step. Rust documents one coalescing per-thread token, early-unpark retention, and spurious returns. The token is shared by **all** uses of parking on that OS thread: unknown code between enrollment and `park`, including logging, can consume it in an unrelated park. Keep that interval free of callbacks or potentially parking instrumentation. `unpark`/token consumption provides synchronization, but a spurious return cannot be used as evidence of publication. A queue mutex or independently justified atomic publication protocol must still protect payload access. [Rust parking contract][R02]

These are candidate reductions in mechanism, not claims that standard primitives are always faster. An internal notification API could expose distinct operations such as publishing runnable work, requesting service and changing dispatch permission, while storing compact shared state underneath. Typed methods can restrict which transition a caller requests; the runtime still has to combine concurrent reasons, preserve service ownership and recheck current state. Rust ownership does not prove liveness, fairness or automatic process preemption.

## 7. Four performance hypotheses

Use the same persistent queue, payload ownership, admitted computation, work accounting and correctness assertions for each comparison. Record compiler/profile, host topology, worker count, affinity, background load and raw repeated samples. Report throughput, publication-to-service and ready-to-run tails, idle CPU, wake operations, contention, timer lateness, kill-to-stop and cleanup delay separately. A microbenchmark without timer/control/reclamation traffic qualifies only notification cost in that subset.

| Experiment | Baseline → treatment and mechanism hypothesis | Required controls / rejection |
|---|---|---|
| **WS54-T01 — Dedicated worker notification** | Correct mutex queue plus predicate-loop Condvar → same mutex queue with dedicated-worker sticky park/unpark; zero spin in both. Test whether removing explicit event ownership/reset changes wake cost or storage. | Force publication before enrollment, between check and park, during park and after wake; include bursts, silence, spurious returns and shutdown. No unknown code in the enrollment-to-park interval; no job count inferred from tokens. Reject lost work, duplicate ownership or apparent gains from a broken Condvar baseline. A foreign token consumer is a required negative control. |
| **WS54-T02 — Coalesce service reasons** | Notify after every service publication → reason-bit transitions or an owned service-generation protocol that coalesces redundant wakes. This focuses the auxiliary/direct distinction, not report 35's spin/recruitment tuning. | Keep runnable work distinct from auxiliary service and dispatch permission. Race the last service drain/clear with new publication; include final small batches and timer-only work. Measure notification count and idle CPU against service age. Reject lost final work or suppression of a required direct queue wake. |
| **WS54-T03 — Transfer offline service ownership** | Suspended workers retain their local timer/auxiliary service → explicit transfer to an eligible service owner before deeply parking/removing those OS workers. Hypothesis: lower idle thread/resource cost at low load. | Conditional later worker-pool experiment. Preserve timer identity/deadlines, arriving queue evacuation, retained bound records, thread-progress participation and exactly one owner per outstanding action. Measure transfer cost, wake latency, timers and retained cleanup under repeated shrink/grow; reject merely hiding service on an unmeasured thread or dropping obligations at shutdown. |
| **WS54-T04 — Explicit controller request ownership** | Dynamic flags plus heap-allocated requester-list records → private typed request/acknowledgment states with runtime-owned indexed records and reusable storage. Hypothesis: fewer allocations and repeated state decoding under resize/block churn. | Conditional on implementing these controller facilities; no initial public API requirement. Preserve each caller's old-count/result semantics, nested blockers and right-in-transit death handling. Include many concurrent requesters and exits. Measure allocation count, retained records and completion tails; reject illegal coalescing of distinct caller observations, ABA/reused identities or gains too small to justify complexity. |

## 8. Seven proposed branch cases

Reserved following `EXEC-07.C21`: **EXEC-07.C22–C28**. These are planned branch qualifications, not claims of executed ERTS coverage. Multi-worker/controller variants remain conditional; the underlying no-lost-work and exclusive-execution obligations apply wherever enabled.

| Case | Trigger and required / forbidden outcome |
|---|---|
| **EXEC-07.C22** | Publish ordinary queue work before enrollment, around event reset/destination CAS, immediately before physical wait and during wait. Enrollment stays ordered with the protected empty decision; wake state or a retained event prevents blind sleep with unserviced work. Spurious wake never grants a process execution claim. [S01]–[S10] |
| **EXEC-07.C23** | Publish auxiliary reasons without runnable application work, repeat/coalesce the same reason and race final reason clearing with new publication. Preserve idle-service continuation while servicing pending reasons; direct queue wake may end the wait loop. Include thread-progress blocking during poll sleep so every installed wake destination is signaled. Forbid treating a poke as dispatch permission or losing the final service batch. [S06]–[S08], [S11]–[S12] |
| **EXEC-07.C24** | Race dirty-worker sleeper-list enrollment, waker extraction, self-removal and suspension; include an empty list and work assigned just before online decrease. Each list record has one removal owner and eligible work retains a servicing worker. No stale links, duplicate ownership or assumption that one coalesced wake equals one job. [S03], [S08], [S14], [S17], [S27] |
| **EXEC-07.C25** | Keep auxiliary work arriving while a timer becomes due; interrupt a timed wait repeatedly; take the normal timer owner offline while more work arrives. Preserve deadline recomputation, timer service, auxiliary ownership and evacuation through suspended service. Forbid offline meaning service destruction, or each interruption restarting the full original duration. [S08], [S15]–[S18], [S28] |
| **EXEC-07.C26** | Increase/decrease online counts during normal-only and full multi-scheduling block, with queued/held work and delayed acknowledgments. Requested, acknowledged and dispatch-enabled states remain distinct; a poke updates controller acknowledgment without releasing an outstanding block. Forbid a premature all-workers-settled result or stranded held records after legitimate resume. [S15]–[S18], [S20]–[S21], [S29], [T01] |
| **EXEC-07.C27** | Queue several controller requests; kill the next requester while the change right is in transit. Separately nest both block kinds and kill a blocker with repeated owned records. Remove only owned requests, preserve remaining blocks and pass the right onward. Forbid controller deadlock, release of another process's block or restart of dead application work. [S20]–[S25], [T02]–[T03] |
| **EXEC-07.C28** | Under full multi-scheduling block, present normal/dirty CPU/dirty I/O work at different/equal priorities, including empty normal work with dirty work pending. Serialized permission changes precede wake; normal service remains reachable for I/O and dirty work is not stranded by idle normal sleep. Forbid parallel dispatch authorization, interpreting `MISC_OP` as an application message, or assuming blocked multi-scheduling cancels dirty work. [S01], [S19], [S21] |

## 9. Evidence inspected and remaining boundaries

`scheduler_suspend_basic_test` checks normal-only/full-block composition with online changes and explicitly qualifies asynchronous dirty acknowledgment. The stress helpers repeat nested blocks and online changes; the concurrent online-change test creates 2,500 requesters doing ten changes, and its exit variant kills selected requesters and waits for termination. These are valuable regression intentions, but reading them does not force the event-reset window, combined poll/TSE state or right-in-transit exit. [Basic composition][T01] [Concurrent requests/exits][T02] [Nested stress helpers][T03]

Traced here: normal sleep enrollment, direct versus auxiliary wake, destination routing, Linux event branch, dirty sleeper-list removal, ordinary/suspended service loops, principal online-change queueing/acknowledgment and multi-block composition. Still open: full weak-memory proof and all event/poll backends; every controller/exit interleaving; full CPU-binding interaction; shutdown/halt behavior; ownership transfer for physically destroyed workers; bounded auxiliary/evacuation work; worker failure recovery; and qualification against an admitted Rust execution engine. No observed or proposed count is a real-time progress theorem.

## Primary source anchors

Local entry points: [worker wait](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:3452), [wake routing](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:1576), [suspended service](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:7790), [online controller](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:8228). All OTP links pin the verified commit. Rust documentation checked 2026-10-08; the rendered pages identify Rust 1.99.0.

[S01]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L9881-L9945
[S02]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L3028-L3083
[S03]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L3451-L3492
[S04]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.h#L266-L285
[S05]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L3086-L3112
[S06]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L3674-L3694
[S07]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L1575-L1600
[S08]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L3501-L3670
[S09]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/include/internal/pthread/ethr_event.h#L35-L81
[S10]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/lib_src/pthread/ethr_event.c#L107-L191
[S11]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.h#L3020-L3030
[S12]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L1603-L1635
[S13]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L3728-L3743
[S14]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L3745-L3782
[S15]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L7790-L7873
[S16]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L7985-L8066
[S17]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L8356-L8431
[S18]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L8068-L8119
[S19]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L7582-L7737
[S20]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L8227-L8354
[S21]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L7875-L7982
[S22]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L8166-L8225
[S23]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14300-L14358
[S24]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L8483-L8587
[S25]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L8589-L8657
[S26]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L8433-L8480
[S27]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L3914-L3929
[S28]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L3377-L3404
[S29]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L8122-L8155
[T01]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/scheduler_SUITE.erl#L1296-L1387
[T02]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/scheduler_SUITE.erl#L1997-L2050
[T03]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/scheduler_SUITE.erl#L1463-L1533
[R01]: https://doc.rust-lang.org/std/sync/struct.Condvar.html
[R02]: https://doc.rust-lang.org/std/thread/fn.park.html
