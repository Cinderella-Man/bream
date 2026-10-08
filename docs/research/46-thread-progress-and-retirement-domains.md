# Thread progress and retirement domains

Research date: **2026-10-07**. Reference: Erlang/OTP **OTP-29.1.1**, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. **Source review only: no runtime test, model, benchmark or Bream implementation was executed in this pass.** This follows [process lookup](27-process-lookup-reclamation-protocol.md), [buffer publication](30-buffered-signal-publication-protocol.md) and [allocator ownership](40-allocator-ownership-and-rust-runtime-storage.md). Any separately executed Loom counterexample concerns its own Rust abstraction; it does not certify this ERTS protocol or a Bream implementation.

The new result is that reader protection, participant activity, sleeping, global blocking, wake requests and deferred cleanup are distinct protocols. A Rust API could encode their transitions and prevent some escaped borrows. It still needs an explicit account of who can access retired memory, who keeps the reclamation domain alive, and what happens when a participant stops making progress.

## 1. Which participants are covered

The source requires managed threads to report progress at controlled states frequently. Registration allocates or uses participant state, installs callbacks, and waits for the configured **entire managed set** to register before returning. The backing arrays are permanently allocated for configured managed/unmanaged counts. This is a VM-lifetime arrangement, not a ready-made registration/removal API for arbitrarily many independently dropped Rust runtime instances. There is no managed-unregister entry point in the inspected header. [Participation contract][S01]; [allocation][S02]; [registration barrier][S03]; [API declarations][S04].

Unmanaged *registration* grants a wakeup callback slot. It does not make the thread an implicit protected reader: unmanaged access uses `unmanaged_delay`/`continue`, even when registered. Conversely an unregistered unmanaged caller can use delay protection; debug lock checking installs temporary thread data for that path. The managed wrapper returns a special handle without incrementing either unmanaged counter because managed participation already supplies its protection. These capabilities must not collapse into “this is one of our threads.” [Unmanaged registration][S05]; [wrapper distinction][S06]; [delay entry/exit][S07].

The reclamation contract is conditional: first remove discoverability, then wait for the relevant progress, with readers dropping temporary references before returning to the controlled state. Thread progress alone does not make a raw pointer safe to load before entry, make concurrently mutable data immutable, or revoke explicitly retained references. The reference's own explanation makes this lookup/access/drop discipline a prerequisite. [Reader contract][S08].

| Transition or owner | What changes; what remains an obligation |
|---|---|
| Managed `active(false)` | Decrements active accounting and may relinquish leadership. It does **not** publish the WAITING sentinel. The thread's confirmation still participates. [S09] |
| Managed `prepare_wait` | Decrements block participation and publishes WAITING with a full-barrier store. It must occur at a safe state; no unretained borrow may silently survive it. [S10], [S08] |
| Managed `finalize_wait` | Publishes a confirmation beyond observed global progress, retrying if global progress changes; re-enters block accounting and honors blocking/code barriers before continuation. [S11] |
| Unmanaged delay handle | Identifies the particular counter increment to release, including failed enrollment retries. Registration or thread identity is not a substitute. [S07] |
| Retired object | Is undiscoverable to new readers but remains owned until old readers and any independent retained owners permit destruction. Progress eligibility does not run its cleanup callback. [S08], [S24] |
| Pending wake callback | Is a notification obligation, not a retained application process or an exactly-once cleanup action. Unregister does not join every pending wake. [S17], [S19] |

Normal scheduler sleep explicitly calls `active(false)` and then `prepare_wait`; wake calls `finalize_wait`. Dirty schedulers bypass those managed transitions in the same loop. This supplies a concrete caller check for the distinction. [Scheduler sleep/wake][S12].

## 2. The coordinator is more than an epoch counter

`update` acquire-loads global progress; when the participant's confirmation equals it, the participant confirms the next value using a full-barrier store. With NO_LEADER set, an active participant—or one observing zero active participants—can claim leadership atomically. The leader checks per-participant cache-line-aligned confirmations, accepting the next value or WAITING. It remembers the first unfinished scan position for subsequent updates in that tenure. [Confirmation/election][S13]; [layout][S14]; [leader scan][S15].

The leader also checks unmanaged counters. If a nonzero current bucket must become the waiting bucket, it publishes the alternate bucket, issues StoreLoad ordering, and rechecks the old count. A nonzero waiting count prevents advancement. Only the successful path publishes the new global value with a full barrier and handles wake requests. Unmanaged enrollment increments the observed bucket and rereads its selection; a changed selection releases that increment and retries. This is the implementation to model, rather than translating the documentation's explanatory two-counter sketch mechanically. [Leader unmanaged branch][S16]; [entry validation][S07].

`later()` reads a managed participant's **confirmed** value, with a barrier, or an unmanaged caller's global value with a full-barrier read, then adds two while skipping the sentinel. It is not always `global + 2`: the managed confirmation can already be ahead. The extra distance prevents previously issued confirmations from satisfying a newly initiated operation. Values are opaque; wrap comparison only handles the prescribed ranges, and the wake mechanism uses four slots plus a debug check on allowed request distance. Fabricated far-future integers are not valid tickets. [Ticket construction/comparison][S18]; [wake validation][S20].

A stalled managed participant or unreleased unmanaged cohort can retain unrelated retired objects sharing this coordinator; safety does not create a cleanup deadline. Existing LIFE-05.C18/C19 already cover entry races and stalls. The additional concern here is recovery when activity ends: an inactive leader releases leadership and rechecks whether the unmanaged cohort reached zero; the final unmanaged decrement also checks whether all managed participants are inactive with pending wakes. These complementary checks arrange a progress-making wake around that handoff. `prepare_wait` similarly checks for work requiring progress; a deep sleeper wakes scheduler 1 instead of itself. [Leadership handoff][S21]; [last unmanaged decrement][S22]; [sleep wake selection][S10]. This is a traced wake protocol, not an exhaustive weak-memory proof.

## 3. Blocking and wake notification are separate barriers

`erts_thr_progress_block` sets BLOCK and waits for the **managed** block count to reach zero, subtracting the initiating participant when managed. Nested calls increment a local nesting count; only the final unblock releases the global block and wakes eligible participants. A participant leaving ordinary wait re-enters this count and can block before continuing. The blocked-participant path runs the required code barrier on release. This path does not wait for unmanaged-delay counters to reach zero. Therefore a token meaning “managed workers stopped” cannot by itself authorize freeing memory that an unmanaged reader still holds. [Block acquisition][S23]; [participant wait][S25]; [nested release][S26]; [wake re-entry][S11].

The fatal-error variant can return before everybody is blocked and has a separate bounded polling wait. Its stack-allocated temporary state is justified by never completing a normal unblock afterward. It is crash machinery, not an ordinary shutdown/retirement shortcut. [Fatal path][S27].

Managed wake requests require a nonwaiting, non-deep-sleeping caller. Already-reached requests wake immediately; duplicate slot/value requests coalesce. If the caller already confirmed the requested value, the request advances one step so the coordinator cannot consume its slot before publication completes. The explicit guarantee is wake **after** the requested point, not at that exact transition. [Managed wake publication][S17].

An unmanaged requester can be overtaken while publishing. It sets hierarchical bitmap bits and a count, then rechecks progress and self-wakes if necessary. A callback may consequently be redundant; consumers must recheck their condition and must not use callback count as reclamation count. Leader wake dispatch walks managed IDs and unmanaged bitmaps synchronously, with no time budget in those loops. [Unmanaged publication][S19]; [wake dispatch][S28].

Unmanaged unregister sets the stored callback argument to NULL and frees participant thread data; the source explicitly permits a later callback. The sampled async-thread caller immediately returns after unregister, and its callback accepts NULL. This does **not** establish a generic reusable-slot, callback-join or domain-destruction contract. Bream must retain or retire callback-visible state under its own supported shutdown protocol. [Unregister][S29]; [async callback][S30]; [async exit][S36].

## 4. Safe-to-reclaim is not reclaimed

The scheduler owns a FIFO later-operation list. Enqueue initializes the callback, argument, target progress and next pointer, appends the caller-provided node, and sets auxiliary work. Ordinary later operations request progress wakeup; cleanup variants additionally coalesce by an accounted byte threshold. These functions require a normal scheduler, not a dirty scheduler or arbitrary host thread. [Enqueue/APIs][S24]; [cleanup wake gating][S31].

The drain first compares its cached current value with the first operation's ticket. Once eligible, it detaches the operation and decrements list length **before calling** it. It must not subsequently depend on storage that callback may free. A callback can also add further work; eligibility and queue ownership must still hold. The dispatch allowance is `list_len / 8 + 50`, so a growing backlog increases the allowance. It bounds callback *count for that invocation*, not callback duration or a fixed scheduler turn. [Drain][S32].

The cached-progress machinery avoids repeated reads within an auxiliary-work pass; a helper self-pokes if a used value became stale, at the applicable end-of-pass path. Cleanup wake thresholds reset when covered by another wake or when the list empties. These branches expose a worthwhile small-final-batch experiment, but this review does not close every early return, scheduler/timer interaction and shutdown path or assert a universal last-batch deadline. [Cached read/recheck][S33]; [pass invocation][S34]; [threshold reset][S35].

A sampled upstream mailbox test waits for thread-progress later work **and then sends a self-message** to establish that a resulting adjustment signal has been handled. That second barrier illustrates why grace completion alone is not completion of downstream effects. Another trace test waits repeatedly for progress to finish breakpoint work. Neither is a general retirement-model test, and neither was run here. [Mailbox barrier test][T01]; [trace helper][T02].

## 5. Rust opportunities and limits

The following are Rust design hypotheses, not features already provided by a selected epoch library:

- **Guard-bound borrowed metadata.** A private guard can expose a reference only for its protected entry scope; higher-ranked callbacks can keep that borrowed lifetime out of the return type. Phantom lifetime/type fields can express relationships the compiler otherwise cannot see. Private construction and either distinct generated brands or validated runtime domain identity must prevent using domain A's guard for domain B's pointer. A lifetime parameter alone does not distinguish two simultaneous domains. [Lifetime bounds][R01]; [PhantomData][R02]. This strengthens the API surface for the existing PERF-046 experiment; it does not repeat that benchmark here.
- **Separate entry, parked participant and retained record types.** Consuming an entry before a safe-point transition could reject using its borrowed view afterward; work that survives the transition must explicitly acquire the appropriate owner first. A thread-affine guard needs deliberate auto-trait behavior. `Send` permits transfer and `Sync` permits shared access; neither says a worker may report quiescence while a suspended task still holds that guard. The standard library's `MutexGuard` is an example of deliberately preventing transfer to another thread, not an epoch implementation to copy. [Send][R03]; [Sync][R04]; [MutexGuard][R05].
- **Runtime-owned participation.** Safe Rust can forget a guard, and destructors are not guaranteed. A leaked guard may safely delay reclamation but fails any unconditional cleanup-completion promise. Force-releasing its protection merely because the associated process was killed is unsound if another authorized reader still exists. Internal guards must stay within qualified runtime work, or a generated operation must track their actual lifetime and stop boundary; host access needs an explicit domain-retention/shutdown contract. [Forget contract][R06].
- **Explicit synchronization proof.** Acquire/release ordering constrains the operations it actually connects; an acquire pointer load is not lifetime protection. Typed state cannot repair an incorrect enrollment/publication/reclamation handshake. ERTS's full barriers, safe states and relaxed observations form one protocol; substituting Rust `Relaxed`, `Acquire` or `SeqCst` operation-by-operation without a complete argument is not a port. [Rust ordering][R07].

These APIs could remove redundant per-object retains for truly scoped readers and avoid dynamic wrong-phase operations. They cannot remove cross-domain access, escaped owners, callback work or allocator latency by naming a type. Reclamation must remain independent of dead application execution, with retired bytes reported separately from live state.

## 6. Nine additional branch cases

Reserved after checking the catalogue: **EXEC-07.C13–C21**. All are **planned, unimplemented and unexecuted**; multi-worker implementation still waits for G7. Existing LIFE-05.C18/C19 and EXEC-03.C16 remain the underlying enrollment/stall cases.

| Case | Trigger and acceptance/forbidden outcome |
|---|---|
| **EXEC-07.C13** | Initialize participants while one registration is delayed; contrast unmanaged wake registration, unmanaged protected entry, and managed participation. No lookup access before valid protection or partially initialized callback use. Rust controls reject the wrong domain/participant capability; no implied dynamic managed deregistration from this reference. [S03]–[S07] |
| **EXEC-07.C14** | Make a managed participant inactive without parking, then park/resume while global progress advances. Inactivity alone never discards its confirmation; resumption publishes beyond observed current before protected work. Reject a borrowed view usable across a claimed quiescent boundary without an independent retained owner. [S09]–[S12] |
| **EXEC-07.C15** | Last active leader relinquishes while the final unmanaged delay ends and retirement wakes remain. Under fair execution, the pending work retains a route to progress; no lost wake or premature advance. Cover deep-sleeper routing and both sides of the final-count recheck. [S10], [S21], [S22] |
| **EXEC-07.C16** | Nest managed blocking, wake a parked participant during BLOCK, and retain an unmanaged protected reader. Only final unblock releases the block; managed stop is not proof of unmanaged retirement. Fatal timeout cannot authorize normal freeing or successful shutdown. [S11], [S23], [S25]–[S27] |
| **EXEC-07.C17** | Obtain tickets when local confirmation equals/is ahead of global; model near-sentinel wrap with reduced-width counters. No ticket satisfied only by confirmations predating initiation; no fabricated ticket accepted as an API value. Keep mathematical wrap-model assumptions explicit. [S18], [S20] |
| **EXEC-07.C18** | Request wake when already reached, when managed confirmation equals target, and during unmanaged bitmap publication. Allow coalescing, delayed and redundant wakes; recheck eligibility and execute each owned retirement action once. Forbid early freeing or counting callbacks as completed objects. [S17], [S19], [S28] |
| **EXEC-07.C19** | Unregister with a pending wake, then retire its callback state; separately close a Rust domain with a retained/forgotten host guard. No stale callback dereference, wrong-domain reuse or false cleanup-complete report. A retained domain may remain safely unreclaimed; unregister alone is not a join. [S29], [S30], [R06] |
| **EXEC-07.C20** | Eligible retirement callback frees its node, enqueues another operation, or performs many nested releases during a large backlog. Detach before invoking; retain ownership of unfinished work and downstream effects. Count-based service allowance is not a duration proof; no application callback escape. [S24], [S32], [T01] |
| **EXEC-07.C21** | Queue cleanup below/at/above wake thresholds; change progress after a cached read, then end with one small batch and no new application work. Qualify the complete wake/flush route, shutdown ownership and truthful cleanup completion. No claim that byte threshold or grace eligibility supplies an elapsed-time deadline. [S31], [S33]–[S35] |

## 7. Five performance experiments

All candidates require correct branch fixtures first. Keep source/artifacts, workloads and raw repeated samples; report throughput, peer-ready delay, kill-to-stop, stop-to-cleanup, retained bytes and CPU separately. Unmeasured hypotheses must not become speed claims.

| Experiment | Baseline → treatment; measurements | Semantic controls |
|---|---|---|
| **TP46-T01 — Confirmation cadence** | Confirm at every qualified runtime turn → fewer confirmations at explicitly eligible turns; W02/W07/W08. Measure barriers/cache traffic, retirement age and maximum observed retained bytes. | No report while scoped references remain; preserve sleep/block/code-barrier obligations. Keep a stalled worker and kill traffic; reject savings that conceal cleanup delay. [S13], [S18] |
| **TP46-T02 — Bounded leader scans** | Scan the remaining managed array in one update → a proved resumable scan allowance; many mostly idle workers. Measure scan time, progress rate, wakeups and scheduler interference. | Preserve every participant confirmation and election/handoff state across slices. No starvation of the final scan entries or assumption that inactive means quiescent. [S15], [S21] |
| **TP46-T03 — Separate retirement domains** | One correct domain for unrelated metadata arenas → independently qualified domains with typed/validated ownership. Measure cross-workload retention under one stalled host reader versus extra bookkeeping and cross-domain handoff cost. | An object reachable across domains must satisfy all relevant protections or transfer a retained owner. Reject arbitrary partitioning, unsafe domain shutdown, or merely hiding retained memory in another account. [S07], [S08], [R01], [R06] |
| **TP46-T04 — Retirement service accounting** | Backlog-scaled callback count → bounded runtime cleanup cursor accounting for nested work; W02/W07. Measure peer tails, callback/release work, drain throughput and backlog. | Preserve detach-before-call and exactly one cleanup owner through kill/shutdown; every callback/final release must be qualified. This specializes PERF-029 to the grace-eligible service queue. [S24], [S32] |
| **TP46-T05 — Cleanup wake policy** | Correct byte-threshold coalescing → qualified byte-plus-age or idle-flush policy; burst then silence and sustained churn. Measure wake CPU, small-batch retention age and peak retired memory. | Record eligibility separately from execution; no busy-wait promise, lost wake, app-dependent cleanup or early free. Establish the baseline's full final-batch route first. [S31], [S33], [S35] |

## Coverage and remaining gaps

**Traced:** principal managed registration/update/election/active/wait paths; unmanaged enrollment/continue; managed/unmanaged wake publication; ordinary block/unblock; later-op enqueue/drain and wake thresholds. **Sampled:** scheduler/aux callers, async unregister, fatal path, and two upstream test helpers. **Unclosed:** complete weak-memory proof and platform atomic implementations, all code-barrier/blocker interleavings, unmanaged callback unregistration synchronization, every caller's retained-pointer discipline, full final-small-batch liveness, runtime/domain destruction, and recovery from a permanently failed managed OS thread. No source branch count is a hard real-time bound. Nothing here chooses an epoch library or establishes that an application process may hold these internal guards across arbitrary native Rust execution.

## Primary source anchors

Local entry points: [coordinator](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_thr_progress.c:660), [wait transitions](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_thr_progress.c:884), [later-op service](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:2281). Rust documentation checked 2026-10-07; the rendered standard-library pages identify Rust 1.99.0.

[S01]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L35-L72
[S02]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L403-L439
[S03]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L593-L655
[S04]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.h#L135-L154
[S05]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L517-L555
[S06]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.h#L199-L217
[S07]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L1008-L1045
[S08]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/internal_doc/ThreadProgress.md#L83-L121
[S09]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L952-L972
[S10]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L884-L909
[S11]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L913-L948
[S12]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L3573-L3611
[S13]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L821-L861
[S14]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L225-L260
[S15]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L678-L721
[S16]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L723-L775
[S17]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L1093-L1162
[S18]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.h#L233-L304
[S19]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L1166-L1210
[S20]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L1059-L1083
[S21]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L784-L813
[S22]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L985-L1005
[S23]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L1337-L1381
[S24]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L2324-L2368
[S25]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L1308-L1333
[S26]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L1441-L1491
[S27]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L1392-L1437
[S28]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L1226-L1283
[S29]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_thr_progress.c#L559-L571
[S30]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_async.c#L370-L395
[S31]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L1688-L1698
[S32]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L2278-L2321
[S33]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L1703-L1734
[S34]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L2759-L2766
[S35]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L1655-L1684
[S36]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_async.c#L422-L441
[T01]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/message_queue_data_SUITE.erl#L331-L348
[T02]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/trace_session_SUITE.erl#L1871-L1877
[R01]: https://doc.rust-lang.org/reference/trait-bounds.html#higher-ranked-trait-bounds
[R02]: https://doc.rust-lang.org/std/marker/struct.PhantomData.html
[R03]: https://doc.rust-lang.org/std/marker/trait.Send.html
[R04]: https://doc.rust-lang.org/std/marker/trait.Sync.html
[R05]: https://doc.rust-lang.org/std/sync/struct.MutexGuard.html
[R06]: https://doc.rust-lang.org/std/mem/fn.forget.html
[R07]: https://doc.rust-lang.org/std/sync/atomic/enum.Ordering.html
