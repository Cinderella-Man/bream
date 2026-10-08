# Process lock ownership and waiter handoff

Research date: 2026-10-07. Source reference: Erlang/OTP `OTP-29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. This pass reads source and selected existing tests; it executes no tests, VM fixtures or benchmarks. It extends [lookup and retention](27-process-lookup-reclamation-protocol.md), [receive parking](31-receive-parking-wakeup-protocol.md) and [scheduler locality](35-scheduler-locality-and-load-balancing.md).

**Record retention, field-access ownership and permission to execute application code are three different capabilities.** ERTS combines them through process state, multiple locks and lifetime protection. A Rust guard can make access obligations more explicit, but it does not by itself establish lock ordering, native preemption, deadlock freedom or safe kill. Initial Bream need not reproduce five locks or parallel workers. The branches below qualify whichever concurrency the implementation actually supports; multiworker and alternate-backend cases are conditional research scope.

## 1. Lock domains and configuration matter

The pinned header defines five lock bits: `MAIN`, `MSGQ`, `BTM`, `STATUS`, `TRACE`. Its declared order is increasing lock kind, then increasing PID for equal kinds on different processes. Thus two-process acquisition interleaves by kind: both main locks precede either message-queue lock. The main lock protects execution state except fields assigned to other domains; the timer, status and trace locks serve different purposes. The transition to `EXITING` requires all process locks, so retaining any one lock can delay that transition. [Domains and order][S01]

The file's introductory prose still says four locks and describes pix locks as spinlocks; current definitions provide five queue heads and initialize pix locks through `erts_mtx_init`. Derive layout and backend assumptions from those definitions, not the older overview. The implementation selects custom packed flags normally, raw mutexes under `VALGRIND` or `ETHR_DISABLE_NATIVE_IMPLS`, and additional atomic/non-atomic variants within the custom implementation. This report primarily traces the custom atomic path. [Configuration/layout][S02] [Introduction and initialization][S03]

These locks are internal synchronization between native runtime threads. A worker waiting on a process lock is not scheduling a green process that called a user-facing mutex. Copying this protocol as an application lock API would introduce a different blocking/admission problem.

## 2. Acquiring a set: fast path, ordered partial ownership, parking

`erts_proc_raw_trylock__` attempts the requested custom lock bits with an acquire compare-and-swap. Success acquires the whole requested set. If a returned flag word intersects a requested bit, it reports contention without acquiring a subset. If the CAS failed only because unrelated flags changed, it retries with the new word. The higher-level blocking wrapper delegates contention to `erts_proc_lock_failed`; the nonblocking wrapper returns `EBUSY`. [Raw try-lock][S04] [Blocking wrapper][S05] [Try-lock wrapper][S06]

The blocking slow path changes the contract: it may **retain a lower-ranked subset while waiting for the rest**. `in_order_locks` stops at the lowest requested bit already held elsewhere; it never grabs requested higher bits past that obstruction. Successful partial acquisition resets its spin counter. When there is no eligible bit, it spins, occasionally yields the native thread, and eventually enters `wait_for_locks`. CAS failures caused by changed flags retry without decrementing that no-progress spin counter. These source counters therefore do not establish a fixed total attempt or elapsed-time bound. [Ordered mask][S07] [Slow acquisition][S31]

Spin settings differ between main runtime threads and auxiliary threads, are zero for one detected CPU, and otherwise depend on CPU/scheduler configuration. The scheduler-thread count contributes to the main-thread setting up to a cap. These are tuning choices, not BEAM fairness semantics or values Bream should inherit. [Spin configuration][S03]

`wait_for_locks` obtains a reusable thread event, records the still-needed mask, takes the process's pix lock and tries again. Under that lock, `try_aquire` acquires available domains in order and enqueues the waiter on its first unavailable domain. If another waiter already occupies that queue, it joins the tail. A waiter belongs to one queue at a time, while potentially already owning earlier domains. If the recheck acquired everything, it leaves without sleeping. [Queue operations][S08] [Under-lock retry][S09] [Wait setup][S10]

That recheck handles the simple release-before-enqueue race: failing an earlier atomic try does not force the thread to sleep after the lock has become available. It is not permission to skip the pix lock when modifying queue links or waiter flags.

## 3. Unlock is sometimes an ownership transfer

The unlock CAS uses release ordering and clears only requested bits with no waiters. Bits with waiters remain set. The slow unlock path takes the pix lock and transfers each such lock to the queue head; another fast-path contender cannot steal a lock by observing it temporarily clear. [Unlock algorithm][S11] [Slow unlock][S12]

`transfer_locks` removes the head waiter, clears that domain from its remaining-needed mask, and tries to acquire its other domains. If more are unavailable, it can enqueue on another domain while retaining the transferred ones. Only when its remaining-needed mask becomes zero is it added to the wake list. The queue update and ownership decision happen under the pix lock; actual event signaling happens after releasing that lock. [Transfer][S13]

| State/event | Required ownership interpretation from this path |
| --- | --- |
| Fast try reports busy | It acquired none of the requested custom lock set. Existing caller locks remain separate. |
| Slow acquisition obtains an ordered prefix | That prefix is owned even though the API has not returned. |
| Waiter moves from one domain queue to another | It has received the earlier domain; moving queues is not releasing all ownership. |
| Unlock encounters waiters | The lock remains unavailable to barging contenders while ownership is transferred. |
| Waiter's needed mask reaches zero | It owns the requested set before its native thread runs again. |
| Event wakes without completed transfer | The wake alone grants no lock ownership; the wait loop continues. |

This is FIFO **within an established domain queue**. It is not a global ordering of lock-call start times: spinning contenders need not have joined a queue, different domains are independent, and the wake list is assembled separately. Nor does it promise an application scheduling order or a deadline for a thread that has been granted locks. Those distinctions matter when measuring fairness.

The wait loop sets a completion flag before releasing the pix lock. Each sleep iteration resets the event, checks that flag, then waits; it retries interrupted waits and explicitly tolerates spurious wakeups. The transferring thread clears the completion flag before setting the event. Consequently, a transfer before the waiter's event reset is still represented by the flag, while a transfer after the check signals the event. Linux event reset includes a memory barrier and event set uses a full-barrier exchange plus conditional futex wake. [Wait/completion loop][S10] [Transfer signaling][S13] [Event wrapper][S14] [Linux event operations][S15]

This is a source trace of the handshake, not a Rust memory-model proof. Event-object reuse/native thread exit, the complete futex wait implementation, non-Linux event variants and every weak-memory interleaving remain unverified here. A replacement must establish its own publication and lifetime ordering; changing the relaxed flag to a Rust atomic without tracing the event barriers is insufficient.

## 4. Lookup must normalize ownership before it can return access

Report 27 already covers PID validation, unmanaged retention and exit revalidation. Three refinements affect a prospective typed interface:

1. **Self lookup subtracts existing locks.** When the requested PID is the caller, `erts_pid2proc_opt` checks its exit allowance, removes already-held bits from the new request and can return immediately when nothing remains. The optional record increment still happens. It must not recursively lock or mint a second exclusive owner for the same domain. [Self lookup][S16]
2. **Two inputs can name one process.** `proc_safelock` coalesces equal-PID inputs, asserts they are the same pointer, and unions the held/required sets. For distinct processes it orders by PID. In lock-check builds, requested final sets must include existing held sets: this helper cannot be used as an implicit permanent release operation. It then finds which higher/equal-ranked locks must be temporarily dropped and reacquires in the global order. [Normalization and contract][S17] [Release/reacquire][S18]
3. **Busy differs from absent.** Nonblocking lookup returns the distinct `ERTS_PROC_LOCK_BUSY` sentinel. Blocking lookup may release the caller's locks, wait, and then reject the target because it exited or left its slot. The returned pointer is not merely the initially observed pointer with locks added. [Lookup contention/revalidation][S19]

For Rust, an operation that may temporarily unlock cannot preserve references derived from the old guard across that gap. A candidate safe API would consume the guard or constrain a callback's borrows, then return freshly validated access. A retained record capability could cross the gap, but could not expose mutable process-body fields. This refines, rather than replaces, existing `LIFE-05.C13–C16` requirements.

Lock-order checking also deliberately tests fallback paths. With checking enabled, `erts_proc_lc_trylock_force_busy` identifies the lowest requested kind and delegates to the global checker. A would-be order violation may be forced busy even when the lock could currently be acquired; by default that forcing is time-dependent, with compile-time variants to disable or always force it. Duplicate locking is checked separately. This makes happy-path try-lock success inadequate qualification of a relock fallback. [Process checker][S20] [Global forced-busy checker][S21]

`erts_proc_lock_wait_until_released` is another easily misread helper: it may acquire and immediately release a selected lock if observed held. It returns **without owning it**. A later holder can reacquire it; the operation is not an enduring unlocked-state assertion. [Wait-until-released helper][S22]

## 5. MAIN ownership is not an execution token

In the scheduling path, the runtime tests active/suspended/running state and atomically sets a running bit **before** dropping the run-queue lock and acquiring `MAIN|STATUS`. After acquiring those locks it rereads process state. Thus a selected scheduler may already own the execution claim while blocked on state access. Conversely, other runtime operations can obtain a main lock without claiming application execution. [Execution claim][S23] [Lock then reread][S24]

Schedule-out clears the applicable running bits through a state CAS and computes the next queue obligation while process locks still protect runtime state. The caller later clears `current_process`/scheduler association and releases `MAIN|STATUS|TRACE`. A subsequent owner can be selected before that final release and wait for the locks; lock handoff alone is not the scheduler's ownership decision. [Schedule-out state][S25] [Release boundary][S26]

A concrete temporary-release example is exact reduction inspection: the current process drops `MAIN`, blocks thread progress to obtain statistics, unblocks it, and reacquires `MAIN`. There is no corresponding schedule-out transition in that helper. This invalidates the simplistic rule that an unlocked main lock always means the application is available to run elsewhere. [Inspection release/reacquire][S27]

The scheduler also has an explicit normal-scheduler exit path while dirty native work remains marked running. It is not a second normal application executor, but it does mean that a global claim of “only one thread ever touches the process” would be false. The native resource/stop limitations remain as documented in reports 14 and 17; Bream's accepted stopped-before-`Down` rule is stricter than treating dirty-native logical death as sufficient. [Dirty exit exception][S23]

**Bream implication, not an implementation selection:** keep an execution claim distinct from lock-domain guards and a retained-record owner. Runtime code must end/revalidate field borrows at lock boundaries. Guards available indefinitely to arbitrary host or application code could delay an all-lock exit transition forever. Memory-safe guard borrowing alone therefore cannot establish prompt control progress, safe forced termination or admission.

## 6. A configuration-specific rollback discrepancy to qualify

The raw-mutex branch attempts domains sequentially and has failure labels intended to unwind acquired domains. At the pinned `busy_trace` label, the code tests the TRACE bit and unlocks `p->lock.trace`, although this label follows a failed TRACE try-lock; the following labels release BTM, MSGQ and MAIN, with no corresponding STATUS release. Under a call that acquired STATUS and then found TRACE busy, this source branch appears to release the wrong domain and retain STATUS. [Raw-mutex rollback][S28]

This is a **source discrepancy requiring targeted qualification**, not a reproduced upstream bug. This pass has neither built the alternate configuration nor established a production caller/interleaving reaching that combination. The custom atomic branch has no such sequential-acquisition rollback. Before treating all configurations as equivalent, construct a native isolated lock-set fixture in the qualifying build, hold TRACE elsewhere, request STATUS plus TRACE, and record actual acquired/released domains. Do not “fix” the reference clone or copy the labels as the oracle. The required Bream outcome is simply that failed acquisition cannot release someone else's lock or retain an undocumented subset.

## 7. Rust leverage and limits

A useful candidate decomposition is `RecordLease`, an internal `ExecutionClaim`, and domain-specific access guards. Guard-bound borrows could prevent supported safe callers from retaining field references after unlocking. However, a main-domain guard should not expose `&mut Process` if other threads may simultaneously mutate its separately locked mailbox/status fields. Project access onto genuinely disjoint domains or use a correctly synchronized interior-mutability representation. Rust's `UnsafeCell` permits interior mutation but does not remove exclusivity requirements of `&mut` or permit data races. [Rust `UnsafeCell`][R01]

`MutexGuard` illustrates scoped access and unlock-on-drop, and is deliberately not `Send` for portability. ERTS transferring internal lock ownership between threads does not authorize moving a standard Rust mutex guard across them. A custom handoff primitive would need its own unsafe implementation and thread-affinity proof. Further, Rust does not guarantee destructor execution; any mandatory runtime cleanup cannot rely on application code eventually dropping a guard. [Rust guard contract][R02] [Destructor limits][R03]

Private typed domains/rank tokens could reject a useful subset of invalid acquisition sequences at compile time. Dynamic PIDs still require runtime ordering or a protocol that avoids simultaneous acquisition. Callback reentrancy, run-queue locks, allocator locks and external calls extend the graph beyond a two-process helper. No combination of `Send`, ownership and typed rank labels proves that whole graph deadlock-free.

Specializing common lock masks through generics may reduce mask interpretation and make ownership sets explicit; Rust uses monomorphization for generic code. Code-size/instruction-cache cost must be measured, and the C implementation already has inline helpers and constant masks at many call sites. Rust's default field layout also does not promise source-order packing, so hot/cold-layout experiments need measured target layouts rather than asserted byte savings. [Rust generics][R04] [Layout contract][R05]

## 8. Five proposed performance experiments

All are unmeasured hypotheses. Use matching workloads and admitted behavior: uncontended operations, many senders to one process, mixed lock domains, cross-process inspection, spawn/kill churn and delayed lock holders. Record throughput alongside spin CPU, parks/wakes, acquisition distribution, per-turn delay, allocation/record size, retained records and stop/reclamation progress. Accept only repeatable benefit without the semantic failures below; no exact latency target is invented here.

| ID | Baseline → treatment and proposed benefit | Required guard / rejection |
| --- | --- | --- |
| **LOCK-R01 — Typed field/rank guards** | Runtime lock-mask accessors → typed domain guards and specialization of common masks. Test whether redundant mask branches disappear without growing code excessively. | Compile-fail attempts to escape a borrow, access the wrong domain and reuse a consumed relock guard. Runtime PID order remains checked. Reject broad `&mut Process` aliases, unsafe escape hatches presented as compiler proofs, or code growth erasing the benefit. |
| **LOCK-R02 — Packed versus separated lock domains** | Shared lock-bit word → separate selected hot domains, or a simpler combined lock for an initial single-worker profile. Hypothesis: reduce cross-domain atomic contention, or reduce uncontended overhead in the simpler profile. | Preserve access and exit-transition atomicity; compare the same concurrency profile. Reject a win obtained by serializing unsupported work away, losing all-domain transition protection, or materially increasing process memory without compensating benefit. |
| **LOCK-R03 — Contention-aware spin/park** | Reference-style fixed main/aux spin settings → bounded/adaptive retries informed by measured hold duration and oversubscription. Hypothesis: spend fewer cycles on locks whose holders cannot run. | Preserve queue/transfer handshake and control progress. Measure CAS-only contention separately because the existing spin counter does not bound it. Reject barging/starvation or hiding native worker blocking behind scheduler-reduction counters. |
| **LOCK-R04 — Owned multi-lock rollback/handoff** | Dynamic acquired-bit bookkeeping → explicit owned lock-set states that can transfer a completed acquisition or unwind only domains actually acquired. Hypothesis: simplify rollback and specialize frequent sets. | Same-process coalescing, every failure position, partial-prefix parking, transfer-before-wait and relock revalidation must match. Use the raw-mutex discrepancy as a negative fixture; do not require cross-thread transfer of `std::MutexGuard`. Reject accidental release of foreign ownership or dependence on user destructors for mandatory cleanup. |
| **LOCK-R05 — Cold waiter metadata** | Queue heads embedded in every process → an optional cold contention record or alternative compact indexed representation. Hypothesis: improve process density/cache locality for mostly uncontended processes. | Account for first-contention allocation and record lifetime through queued/granted-but-not-resumed waiters, exit and shutdown. Reject allocating an unqualified resource while holding required runtime locks, losing a waiter during record retirement, or worse tail latency hidden by average memory savings. |

## 9. Proposed additional cases

Eight new refinements: `EXEC-01.C16–C23`. Existing `LIFE-05.C13–C16` cover general lookup retention, unlocked gaps and revalidation; existing `EXEC-01.C11–C15` cover receive/wakeup state. These new cases focus internal lock-set and worker handoff branches. Multiple native workers/backend variants are conditional, not initial product requirements. Initial code still must have valid field-access and execution ownership.

| Case | Trigger and required outcome | Forbidden outcome / evidence |
| --- | --- | --- |
| **EXEC-01.C16** | Change unrelated lock flags during whole-set try acquisition; separately occupy each requested domain. Return either the full newly requested set or busy with none of that set acquired. Keep pre-existing caller ownership distinct. | Partial ownership escaping a failed nonblocking atomic try, or busy reported as absent. [S04], [S06], [S19]. |
| **EXEC-01.C17** | Block a middle requested domain while lower and higher domains are free. Slow acquisition can retain only eligible lower domains and wait on the first unavailable domain; repeat with CAS interference. | Acquiring higher domains across the obstruction; claiming a fixed overall wait bound from a counter that ignores CAS-only retries. [S07], [S09], [S31]. |
| **EXEC-01.C18** | Queue multiple waiters on overlapping lock sets and release several domains. Transfer queue-head ownership, move incomplete waiters to their next domain, and wake only completed sets. | Barging into a still-owned transferred bit, duplicate domain owners or treating per-domain FIFO as global application fairness. [S08], [S11], [S13]. |
| **EXEC-01.C19** | Transfer ownership before event reset, between reset/check, between check/wait, and during interrupted/spurious waits. Completion state survives all paths; event wake alone never grants ownership. | Lost completion/wakeup, premature return or event storage retired while handoff can still touch it. Linux event ordering is traced; full lifetime/platform proof remains open. [S10], [S13], [S14], [S15]. |
| **EXEC-01.C20** | Self lookup requests already-held and additional domains; two-process helper receives the same process twice with overlapping masks. Coalesce ownership and acquire only missing domains; maintain required final set. | Recursive acquisition, duplicate exclusive guards or helper silently releasing a caller-required domain. [S16], [S17], [S18]. |
| **EXEC-01.C21** | Pause after scheduler running-state claim but before MAIN acquisition; separately temporarily release MAIN without schedule-out and delay final schedule-out unlock. Another worker must not infer application permission merely from lock availability. | Concurrent normal application execution, use of stale unlocked field borrows, or copying dirty-native death behavior as proof of stopped-before-`Down`. [S23], [S24], [S25], [S26], [S27]. |
| **EXEC-01.C22** | **Conditional backend qualification:** force failure at each sequential mutex acquisition position, especially STATUS acquired then TRACE busy; compare custom atomic and raw-mutex configurations. Failure owns/releases exactly the specified domains. | Unlocking a domain not acquired, retaining an undocumented subset, or treating one backend's tests as all-backend evidence. Pinned TRACE rollback is an unconfirmed discrepancy to resolve. [S02], [S28]. |
| **EXEC-01.C23** | Force try-lock busy when the fast path could otherwise succeed out of rank, including two PIDs in each order and existing locks. Fallback restores required ownership in valid order; revalidate after released protection. | A fallback that deadlocks only under real contention, or treating passing timed forced-busy checks as exhaustive verification. Compile-time rank checks do not replace dynamic PID/whole-graph checks. [S17], [S18], [S20], [S21]; dependencies `LIFE-05.C13–C16`. |

## 10. Inspected evidence and explicit gaps

Source tracing covered custom atomic acquisition/release, queue insertion/transfer, event completion handshake, same-process normalization, principal lookup fallback and selected scheduling boundaries. Alternate raw mutexes were sampled at their rollback path, not fully qualified.

`process_SUITE:process_info_lock_reschedule` repeatedly inspects running and waiting processes and checks the running target remains schedulable. Its second variant creates reciprocal and many-to-one inspection pairs and waits for all to finish. These are useful contention/deadlock regression workloads, but do not force every internal waiter interleaving or qualify the alternate TRACE rollback. [Upstream tests][T01] [Reciprocal inspection][T02] The lock-check build also checks waiter-list/flag consistency and per-domain ownership counts. Those assertions are evidence of intended invariants, not proof in builds or schedules not exercised. [Queue assertions][S29] [Owner-count assertions][S30]

Remaining work includes a finite model of partial-set handoff and event publication; compile-fail guard/relock probes; native pause points for each handoff/failure branch; event reuse and thread-exit ownership; all backend/memory-order variants; an audit of the complete lock graph including callbacks and allocation; panic/shutdown behavior of a Rust primitive; and evidence that admitted code cannot strand a runtime lock when forcibly stopped. Nothing here selects a lock crate, an execution engine or a production layout.

## Source anchors

All OTP links are immutable. Local entry points: [fast path](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process_lock.h:596), [handoff](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process_lock.c:283), [ordered relock](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process_lock.c:581), [lookup](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process_lock.c:824), [scheduler claim](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:10068).

[S01]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.h#L112-L201
[S02]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.h#L43-L108
[S03]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L35-L147
[S04]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.h#L582-L617
[S05]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.h#L657-L730
[S06]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.h#L824-L890
[S07]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L362-L382
[S08]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L183-L223
[S09]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L225-L274
[S10]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L393-L462
[S11]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.h#L735-L802
[S12]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L544-L561
[S13]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L276-L359
[S14]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_threads.h#L2473-L2491
[S15]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/include/internal/pthread/ethr_event.h#L52-L81
[S16]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L847-L879
[S17]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L573-L698
[S18]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L700-L803
[S19]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L917-L987
[S20]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L1395-L1421
[S21]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_lock_check.c#L1059-L1129
[S22]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.h#L988-L1028
[S23]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10068-L10193
[S24]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10189-L10240
[S25]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L6741-L6798
[S26]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L9730-L9785
[S27]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12117-L12136
[S28]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.h#L619-L654
[S29]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L1678-L1708
[S30]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.h#L893-L915
[S31]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L474-L542
[T01]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L936-L969
[T02]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L971-L1021
[R01]: https://doc.rust-lang.org/std/cell/struct.UnsafeCell.html
[R02]: https://doc.rust-lang.org/std/sync/struct.MutexGuard.html
[R03]: https://doc.rust-lang.org/std/mem/fn.forget.html
[R04]: https://doc.rust-lang.org/book/ch10-01-syntax.html
[R05]: https://doc.rust-lang.org/reference/type-layout.html
