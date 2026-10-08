# Process execution, memory and owned state: deeper source audit

**Execution follow-up:** [Report 29](29-pinned-reference-observations.md) now records targeted observations on the exact pinned build. The source-only status and hypotheses below describe the original review stage; the follow-up identifies which variants were executed and which remain unresolved.

Reviewed 2026-10-07 against the official local checkout of Erlang/OTP `OTP-29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. All upstream links pin that commit. No source was changed, VM built, or test executed. This extends [report 11](11-beam-scheduling-source.md), [report 14](14-beam-memory-native-io-source.md) and [report 17](17-execution-resource-edge-cases.md); it does not turn their sampled review into an exhaustive audit.

The new findings concern state transitions that a superficial “process plus queue” model misses: multiple queue records can represent one process; suspension does not suppress all runtime work; sleeping workers use a race-sensitive handshake; GC can retain abandoned heaps while moving the stack; changing mailbox allocation mode has a transitional period; and hibernation has two different continuation contracts. These are directly about processes. The only OTP behaviour code inspected is the small `hibernate_after` call site needed to distinguish a library policy from a VM facility. No bundled compression, cryptographic or other utility algorithm was reviewed.

## Coverage and evidence strength

| Area | Coverage in this pass | What remains outside the inspected path |
|---|---|---|
| Enqueue/claim/schedule-out | Traced priority selection, proxies, atomic claim, suspended/system-work and inactive-free branches | Complete proof of all producers, memory ordering and dirty/normal interleavings |
| Worker sleep/wake | Traced preparation, sleep-type installation, wake flags, poll/event dispatch and interrupted wait | Every OS event/poll implementation and all auxiliary-work producers |
| Balancing | Traced stealing selection/transfer and migration publication boundaries; sampled balancing entry/exit | Middle policy calculation, NUMA/affinity strategy, complete scheduler evacuation and online/offline protocol |
| Explicit process suspension | Traced public validation/counting, immediate versus signal path and resume request | Full suspend-signal consumer and suspender-death monitor teardown; adjacent signal/lifecycle reports own those |
| Heap/GC | Traced dispatch, roots, major collection, minor fallback, delayed allocation, limits and off-heap release | Every term-tag copier, all literal-purge cases, all writable-binary shrink heuristics and allocator implementations |
| Mailbox storage modes | Traced setter, publication delay, conversion/death/reversal branches and GC roots | Queue delivery/order/alias semantics, covered by the signal/queue audit |
| Hibernation | Traced `/0` interpreter/x86 lowering, `/3` BIF, common wakeup handshake and compacting collection | ARM JIT and every debugger/code-loading interaction |
| Process dictionary | Traced ownership roots, replacement, return-value protection, snapshot copying and teardown; sampled grow/shrink | Complete hash algorithm/performance audit and every key shape |

“Traced” means the named call chain and listed branches were read; it does not mean all transitive callees were audited. Upstream test assertions are source evidence, not reproduced results.

## 1. Queue membership and permission to execute are separate

The useful chain is `change_proc_schedule_state` → `check_enqueue_in_prio_queue` → `select_enqueue_run_queue` → queue insertion, followed by `erts_schedule`'s atomic claim and finally `schedule_out_process`. [State-change restrictions and CAS](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L6899-L7002), [enqueue classification](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L6570-L6684), [local source](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:6570).

| Branch | Inspected action | Consequence |
|---|---|---|
| Suspended with neither ordinary nor dirty system work | `check_enqueue_in_prio_queue` declines enqueue | Suspending application execution is distinct from suppressing runtime control work. |
| Already represented at the same or higher effective priority | Declines another enqueue at that priority | Avoids unnecessary scheduling records without requiring one record globally. |
| Already queued lower, or effective priority differs from user priority | Reserves another priority bit and requests a proxy record | Multiple queue records can refer to the same real process. |
| Dirty work requests termination | Dirty enqueue helper routes termination to a normal queue | Death processing does not depend on a dirty native call returning first. |
| Inactive and `FREE` | General state-change helper does not schedule it | A leftover queue record is not evidence of a live executable process. |

The dirty-queue helper additionally resolves competing enqueue reservations through `xstate`; if another thread already queued the work, it can clear the reserved real-process slot and adjust the exiting process's reference count. This is a lifetime branch, not simply a throughput optimization. [Dirty enqueue/reservation cleanup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L6492-L6565).

`erts_schedule` resolves proxies to the real process, clears the consumed queue bit, checks active/suspended/running state and attempts a CAS setting an execution bit. A losing or obsolete record is discarded. The exceptional simultaneous normal/dirty activity is narrowly defined: a normal scheduler may perform exit processing while `DIRTY_RUNNING` application native work remains, **not** while `DIRTY_RUNNING_SYS` is active. This is not two application continuations. [Claim and discard branches](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10046-L10185).

On schedule-out, the appropriate normal/dirty running bits are cleared, work flags are reconsidered, and the CAS result determines requeueing. The comment identifies this as the place where `ACTIVE_SYS` is cleared once its relevant work flags are absent. Work arriving from another thread has a subsequent activation/wakeup path; the scheduler can notice its preliminary flags and avoid waiting for that later wakeup. [Schedule-out race handling](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L6724-L6837).

**Bream consequence:** retain the invariant “one application execution owner,” rather than infer “exactly one queue record.” Bream can choose simpler queues, but stale records, execution claims and allocation lifetime still require separate reasoning. Its accepted `Down` contract excludes continuing application native work after notification; copying BEAM's dirty-exit exception would violate that contract.

## 2. Priority policy includes effective priority and skipped selections

`enqueue_process` appends FIFO within its chosen queue. Low-priority processes share the normal queue but receive `RESCHEDULE_LOW` as `schedule_count`; `check_requeue_process` rotates a low entry while its count remains positive **unless it is the only entry**. Maximum and high work use separate queues. [Queue insertion and rotation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L3943-L4053).

`erts_set_process_priority` distinguishes user priority from effective priority. Pending system-task queues, including delayed GC work, can keep effective priority above the newly requested user priority. This is runtime-task scheduling machinery; it must not be advertised as general application-lock priority inheritance. [Priority setter](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L9456-L9546), [public priority contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L8198-L8256).

The `low_prio` regression deliberately uses a high-priority sink to avoid changing the measured normal/low workload and mailbox-related charging. It checks a broad range around an approximately 1:8 ratio, not exact global round-robin order or a latency bound. [Test](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L2138-L2203). Priority APIs remain a later reference area for Bream; normal-priority peer progress is the current requirement.

## 3. Sleeping workers and moving work have explicit handshakes

`scheduler_wait` enters with the run queue locked, prepares sleep flags, records waiting status and unlocks. It then considers auxiliary work, timer deadlines and either poll-based or event-based sleep. Event waits retry `EINTR`; normal schedulers bracket relevant waits with thread-progress state changes. Dirty schedulers take different branches and maintain a sleepers list. [Wait loop](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L3452-L3635), [local source](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:3452).

The race-sensitive part is not the eventual OS wait call. `sched_set_sleeptype` resets the event or poll interrupt, then CAS-installs a sleep type only while `SLEEPING|WAITING` still holds. A concurrent wake can clear those flags and prevent sleeping. The wake path CAS-clears waiting/sleeping while preserving suspension/multi-scheduling bits; `erts_sched_finish_poke` uses the previous sleep type to interrupt poll, set the event, or both. [Prepare/install](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L3029-L3112), [wake flags](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L3674-L3743), [actual poke](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L1576-L1599).

Stealing first checks whether the destination is suspended/inactive and whether other queues can have work. It searches inactive queues, active queues, then revisits contended candidates, finally reacquiring its own lock and rechecking whether work arrived there. A victim batch is at most half rounded upward and capped at 100. Assignment transfer fails for a bound process. After releasing the victim lock and reacquiring the destination lock, insertion explicitly accounts for work another producer may have appended meanwhile. [Victim transfer](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L4468-L4595), [candidate/recheck path](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L4640-L4715), [bound-assignment CAS](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.h#L2701-L2732).

`check_balance` serializes calculations, treats a one-queue system specially, and coordinates its update mutex with multi-scheduling changes. New migration paths are published atomically, offline entries retained, queue statistics reset and old paths retired. The policy calculation between these inspected boundaries was **not** fully audited. [Entry](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L4894-L4959), [publication/retirement](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L5411-L5472). Steal limits constrain one critical section; they do not prove an end-to-end scheduling deadline.

## 4. Explicit suspension is counted, owned and separate from preemption

`suspend_process` can set the flag immediately for a non-running target using CAS; a running or exiting target prevents that immediate path. Successful or already-active suspension increments `rcount`. Internal `resume_process` decrements it and clears `SUSPENDED` only when the count reaches zero, then reevaluates enqueueing. [Internal primitives](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L7235-L7325).

The public BIF adds ownership through suspend monitors. It rejects self/nonlocal identifiers and malformed options, records per-caller counts, handles count overflow, supports `unless_suspending`, and tries a locked immediate suspend before sending a monitor signal. Missing targets remove the newly created relationship; synchronous and asynchronous outcomes differ. Synchronous signal handling uses a correlation reference/save position rather than assuming the signal was already processed. Resume without an owned suspension fails; the last owned resume removes the origin record and sends a demonitor signal. [Public suspend branches](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L9013-L9173), [resume](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L9181-L9212).

The official contract permits an asynchronous request to be resumed before it ever suspends, with tagged replies distinguishing `exited`, `suspended` and `not_suspended`. A suspender's death releases its suspension counts; cycles can deadlock, and this BIF is intended for debugging. These contract branches are known, while their complete signal-consumer/death implementation is outside this section's trace. [Contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L6275-L6360).

Suspension is also **not** a statement that all process-associated runtime work or time continues normally. `ACTIVE_SYS` can execute despite suspension; `system_task_on_suspended` explicitly collects a suspended process without resuming application execution. Receive timeouts pause as of OTP 28, and PID-addressed BIF timers as of OTP 29; name-addressed timers are excluded. The immediate suspend path calls both timer-pause helpers. Detailed timer reconstruction/identity semantics belong to the signal/timer audit. [Suspended-GC test](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L5063-L5077), [versioned timer contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L6275-L6283).

This public debugging facility is not required to achieve Bream's automatic scheduling. If added later, it needs this distinct ownership/progress contract.

## 5. Collection depends on a complete root and relocation model

`setup_rootset` includes the stack, dictionary bucket terms, supplied live registers, sequence token, non-immediate group-leader/parent values, failure reason, stored stacktrace, receive-marker references and saved native-function arguments. Queue references are roots unless off-heap management is fully settled. In transitional signal representations, some slots are deliberately excluded because they do not yet contain valid Erlang terms. Root metadata can itself allocate when queues exceed the default root array. [Roots](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L2615-L2783), [local source](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_gc.c:2615).

`full_sweep_heaps` prepares the root set before overwriting old terms with move markers, updates root references, scans copied terms, sweeps off-heap ownership and frees the old generation. Its comment intentionally preserves a healthy old heap until allocation has finished, so allocation-failure diagnostics do not inspect a half-forwarded heap. `copy_erlang_stack` moves the Erlang stack and, under the frame-pointer layout, relocates applicable frame pointers. [Full sweep](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1963-L2055), [stack movement](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1579-L1632).

Normal collection dispatch is conditional:

1. `F_DISABLE_GC`, `F_DELAY_GC` or exiting state takes delayed allocation.
2. A normal scheduler can classify the work as dirty before collection starts.
3. Below `max_gen_gcs`, with no requested full sweep, it attempts minor collection.
4. Insufficient old-generation/virtual-binary capacity returns `-1`; the caller falls back to major collection, potentially reclassifying it as dirty.
5. A kill-enabled heap-limit hit returns `-2`, marks self exiting with `killed`, clears GC state and supplies the requested space through delayed allocation so the calling path can finish safely.

[Dispatch/error paths](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L759-L898), [minor preconditions](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1398-L1514), [minor fallback](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1552-L1576).

Major collection sizes a new heap for live generations plus stack/reserve, moves data, resets the minor count, restores high-water state and performs post-collection sizing. Its return value accounts for moved/resized words. `gc_cost` divides these counts and clamps the result; this is accounting after work, not an interruption mechanism inside arbitrary copying. [Major flow](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1855-L1959), [cost calculation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L218-L229).

**Rust boundary:** ordinary Rust stacks/objects do not inherit this tagged-root relocation contract. This research adds ownership obligations for Bream's admitted values; it does not select a moving GC, permit arbitrary stack copying, or establish safe reclamation of untracked Rust destructors.

## 6. GC disabled, minimums and maximums mean specific things

`delay_garbage_collection(need == 0)` can return immediately, except pending dirty GC forces rescheduling. With positive need it allocates a heap fragment, moves the stack, retains the prior heap/fragment in the process's ownership, preserves its high-water marker, and sets `F_FORCE_GC`. It charges the remaining turn. GC disabled therefore does not mean allocation disabled, memory usage fixed, or collection requests ignored. [Delayed allocation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L500-L603).

System GC requests encountered while GC is disabled are saved rather than answered as completed. Reenabling splices delayed requests ahead of existing requests of the same priority because they already reached the head once. The enable/disable helper asserts against accidentally nested disable scopes that could reenable prematurely. [Task deferral](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10818-L10857), [reenable](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L11823-L11926). Exit phases also change GC availability to protect the exit reason; the lifecycle report owns that phase order.

Settings have distinct branches:

- `min_heap_size` and `min_bin_vheap_size` reject invalid/negative inputs, apply implementation minimums and otherwise round to a supported heap size. Changing the flag is not an immediate general compaction request. [Setters](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L2114-L2146).
- `fullsweep_after` affects minor-versus-major selection; zero takes the major path. The setter accepts a nonnegative small integer, while the stored field is `Uint16`. Values around and beyond that width deserve an executed boundary fixture before documenting a complete numeric-domain promise; this pass does not promote the apparent truncation into desired Bream behavior. [Setter](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L2101-L2112), [field](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.h#L1141-L1145).
- Maximum-heap checking compares **strictly greater than** the limit and can include virtual shared-binary accounting. Minor/major estimates include candidate allocation and still-existing heap/stack space; this is not simply live payload bytes. A zero limit bypasses these checks; log and kill flags are distinct. [Comparison](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L3730-L3754), [minor estimate](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1431-L1456), [major estimate](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1891-L1905).

These knobs describe a GC representation Bream has not chosen. They are reference/research branches, not authorization to impose process quotas on the first Rust library.

## 7. Mailbox allocation mode has a conversion lifetime

When a received message has attached fragments, `erts_save_message_in_proc` transfers their off-heap lists and accounting to the process and retains their storage in `msg_frag`; a message already referencing process storage needs no attached-fragment transfer. Thus consuming the queue record does not necessarily free the payload's backing allocation immediately. [Receive ownership transfer](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.c#L965-L999).

Changing on-heap to off-heap sets the sender-visible process flag, records change-in-progress and schedules a thread-progress-later operation. That callback sends the process a conversion signal after prior sender activity has crossed the relevant boundary. Off-heap to on-heap flushes/deinstalls queue buffers under the message-queue lock; it cannot clear the sender-visible off-heap bit prematurely if the earlier transition is still pending. [Setter and deferred callback](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.c#L1007-L1134).

The conversion consumer checks death first, checks whether the user reversed the requested state, then visits pre-boundary messages. Eligible heap references are copied to attached fragments. It charges scanning/copying and yields with markers between messages; a single `copy_struct` call still completes before that yield test. Completion destroys continuation markers and clears change-in-progress. [Death/reversal/start](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L8355-L8428), [copy accounting/yield/cleanup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L8460-L8519).

The GC root predicate includes transitional queues until the conversion is complete. Repeated mode flips cannot safely be modeled as simply toggling a boolean and forgetting old references. Bream need not expose these exact modes, but any analogous ownership/layout transition needs a comparable publication-and-reclamation argument.

## 8. Binary and native-resource release can defer physical destruction

During minor GC, off-heap sweeping preserves forwarded references and stops when it reaches old-generation entries; unreachable old references may therefore await a full sweep. Dead young references release binary/magic-reference ownership. Full sweeps examine all relevant entries. [Sweep branches](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L2996-L3110).

`erts_bin_release` atomically decrements the reference count and only calls `erts_bin_free` at zero. Ordinary binary storage frees directly; magic binaries invoke a destructor whose false result means the destructor took control of deallocation. Process/message cleanup walks these references through the same release path. [Reference and destructor dispatch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_binary.h#L366-L395), [cleanup walker](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.c#L165-L205).

The NIF-resource destructor prologue schedules auxiliary work and returns “do not free.” That work marks resource monitors dying, detaches their tree, and returns again if monitor references still exist; the prologue is invoked again when their count permits completion. Only then does it call the user destructor, release the resource-type ownership and free the magic binary. [Resource callback path](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L2957-L3029).

This supplies a concrete distinction between dropping an owning reference, invoking a callback and freeing physical storage. It establishes no bound on arbitrary native destructor duration. Bream's opaque immutable bytes can use a much narrower contract, but release counters must remain truthful when other owners or deferred runtime work exist.

## 9. Hibernation has continuation and wakeup branches

`hibernate/3` performs bounded validation before discarding the stack, catch frames and return-trace state. Invalid module/function types and detected malformed lists take `BADARG`; excessive arity can take `SYSTEM_LIMIT`. The exact boundary is subtle: with `MAX_ARG = 255`, the loop can consume a proper list of exactly 256 arguments, reach `nil` and bypass the early error block, while 257 leaves a nonempty tail and triggers it. This is an unexecuted source prediction about early validation, not a claim about the final resumed result. Add isolated 255/256/257 and improper-tail fixtures; the inspected upstream excessive-arity assertion uses 5,122 arguments and does not cover this boundary. The BIF saves arguments for later `apply/3`, so missing exported code can fail after the old execution state has been discarded. [BIF](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1366-L1427), [arity constant](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_vm.h#L50-L54), [existing error test](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/hibernate_SUITE.erl#L220-L228).

OTP 28 added `hibernate/0`: it preserves the call stack and returns `ok`. The interpreter records the next instruction after the common helper; x86 JIT synchronizes runtime state, calls the same helper and dispatches return. It is not the `/3` contract with three default arguments. [Public distinction](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L3511-L3559), [interpreter](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/emu/trace_instrs.tab#L123-L135), [x86 JIT](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/instr_trace.cpp#L254-L278).

The common helper saves the live arguments, fetches queued signals under message/status locks, and only attempts compacting GC if no waiting messages require execution. After GC it reacquires the locks and fetches again before clearing `ACTIVE`. That second fetch matters for arrival during collection. [Common helper](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_common.c#L1568-L1602).

Compaction can first move to dirty system execution. Otherwise it collects into a temporary heap, allocates an exact final heap plus stack/reserve, copies and offsets references, and clears forced-GC state. The separate exact allocation is intentional to avoid the fragmentation of shrink-by-reallocation. This path can shrink below `min_heap_size`; later ordinary collection restores the minimum. It is a representation-specific optimization, not proof an arbitrary Rust continuation can be relocated. [Compaction](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1004-L1132), [minimum-size contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L3545-L3553).

`hibernate_after` is a higher-level inactivity policy: `gen` reads the option and `gen_server` uses a receive timeout to enter its hibernation loop. It is not an automatic VM rule for all idle processes. Those two call sites were inspected solely to establish this boundary. [Option](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/stdlib/src/gen.erl#L734-L740), [loop](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/stdlib/src/gen_server.erl#L2320-L2326).

## 10. Process dictionary: owned roots, returned values and snapshots

The dictionary is process-associated storage whose bucket terms belong in GC's root set. `put/2` allocates the dictionary lazily, returns the previous value or `undefined`, and can update an immediate replacement in place. A replacement requiring allocation preserves key, value and old return value across GC using an explicit three-element root array. [Put path](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_dict.c#L628-L757).

Internal mutation is not exposed by an earlier `get/0`: `pd_hash_get_all(keep_dict=true)` copies the key/value tuple containers because later put operations may mutate them. `erase/0` can return the pairs without that protective tuple copy because it discards the dictionary. `erase/1` preserves its returned value across shrink-triggered GC. Process teardown frees the dictionary array through `erts_erase_dicts`; term backing storage is owned by the process heap/off-heap mechanisms rather than recursively destroyed by the hash-array free. [Snapshot copy](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_dict.c#L579-L625), [erase operations](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_dict.c#L363-L443), [shrink roots](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_dict.c#L766-L804), [teardown entry](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_dict.c#L232-L239).

Growth is split into a limited number of bucket splits per call, but bucket chains, enumeration and shrink work still contain loops without a scheduling continuation in the inspected functions. Do not convert “hash table” into a constant-latency claim. [Growth](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_dict.c#L852-L931).

A typed Rust process dictionary is not selected. If one is later proposed, state access, snapshot ownership, replacement return values, cleanup and admitted dynamic types need an explicit Rust contract. Runtime-owned process metadata likewise cannot hide roots/resources from the ownership inventory.

## Selected upstream tests and their limits

| Test body inspected | Regression evidence it supplies | Limit |
|---|---|---|
| [`scheduler_suspend_basic_test`](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/scheduler_SUITE.erl#L1296-L1359) | Normal-only versus all-scheduler blocking and online/active counts differ; comments note partly asynchronous transitions. | Sampled first state transitions, not the entire suite or all OS wakeup races. |
| [`simultaneously_change_schedulers_online_with_exits`](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/scheduler_SUITE.erl#L2020-L2050) | Thousands of changers, some killed, must all terminate and permit restoring the setting. | Stress scenario, not a proof of balance policy. |
| [`gc_request_when_gc_disabled`](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L5130-L5162) | Async GC response waits until reenabling, with immediate and reference request IDs. | Uses internal debugging controls and timing waits. |
| [`gc_signal_order`](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/gc_SUITE.erl#L290-L317) | Earlier kill followed by GC request yields false despite preceding link/unlink pressure. | One sender/order setup, not arbitrary concurrent signal order. |
| [`max_heap_size` / `max_heap_size_large_hfrag`](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/gc_SUITE.erl#L187-L235) | On-heap queued messages and large BIF fragments can trigger killed termination at GC. | Does not establish every numeric boundary, allocation-failure branch or hard physical cap. |
| [`change_to_off_heap_gc`](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/message_queue_data_SUITE.erl#L303-L323) | Targets external-queue arrival plus mode change plus GC, especially debug assertions. | Its `DOWN` receive does not constrain the reason; do not overstate it as a complete delivery-success assertion. |
| [`process_info_messages`](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/message_queue_data_SUITE.erl#L118-L166) | Repeated allocation-mode changes preserve the inspected/received message sequence. | Does not enumerate every conversion-yield/death point. |
| [`stuck_dirty_hibernate`](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/hibernate_SUITE.erl#L372-L414) | Monitor/message arrivals near dirty hibernation must not leave the process stuck. | Repeated timing perturbation, not exhaustive race exploration. |
| [`in_place`](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/hibernate_SUITE.erl#L429-L500), [`min_heap_size`](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/hibernate_SUITE.erl#L179-L214) | `/0` retains deep stack/live data and waits; `/3` can shrink below the configured minimum before regrowth. | Heap-size assertions are implementation-specific, not Rust density targets. |
| [`pdict_SUITE:destructive`](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/kernel/test/pdict_SUITE.erl#L374-L402), [`mixed`](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/kernel/test/pdict_SUITE.erl#L404-L457) | Prior snapshots survive immediate replacement; randomized grow/shrink deliberately forces GC during erase. | No exhaustive key/collision/latency coverage. |

## New branch cases for plan integration

These **34 additional case IDs** extend the inventory without changing existing EXEC-01–07 cases. They are source-derived reference/later research cases, not a claim that all named BEAM APIs must ship in Bream's first milestone. For representation-specific behavior, the Bream task is to qualify an equivalent ownership invariant or explicitly document why the API is out of scope. No case below has been executed here.

### EXEC-08 — advanced scheduling and suspension: 12 cases

| Case | Trigger / precondition | Required reference observation or qualification |
|---|---|---|
| EXEC-08.C01 | Low entry has normal peers, then becomes the sole entry | Rotation policy reduces its frequency with peers but does not repeatedly skip the only runnable entry; no hard ratio/deadline promise. |
| EXEC-08.C02 | Change user priority with active/delayed system tasks | Distinguish user/effective/queued priority; preserve pending higher-priority runtime work. |
| EXEC-08.C03 | Priority elevation while a lower-priority entry exists | Proxy and old records must still yield one real application execution owner; stale record disposal releases lifetime references. |
| EXEC-08.C04 | Process becomes inactive-free before a queue record is consumed | Discard the record safely without executing or retargeting it. |
| EXEC-08.C05 | Suspended process receives eligible system work or exit | Runtime work may proceed; application execution stays suspended unless lifetime termination supersedes it. |
| EXEC-08.C06 | Multiple suspenders and repeated owned suspend counts | Only final applicable resume releases suspension; suspender death removes its counts without cancelling other owners' suspensions. |
| EXEC-08.C07 | Async suspend races owned resume or target death | Tagged acknowledgment may be suspended/not_suspended/exited according to the race; sending the request alone proves no immediate suspension. |
| EXEC-08.C08 | Self/remote/dead target, malformed options, absent resume ownership, count overflow | Preserve documented error distinctions; overflow must not leak an increment. Dangerous count/deadlock tests require isolated fixtures. |
| EXEC-08.C09 | Wake arrives before/after sleep-type publication or an OS wait is interrupted | No lost wakeup; correct poll/event poke; interruption alone must not lose timer or runnable work. |
| EXEC-08.C10 | Steal encounters bound tasks, contended victims or concurrent destination enqueue | Bound assignment remains protected; transferred list/counts stay consistent and no process executes twice. |
| EXEC-08.C11 | Online/active scheduler configuration changes while requesters exit | Complete or release ownership consistently; distinguish normal/dirty counts and partial asynchrony. |
| EXEC-08.C12 | Suspended receiver has receive timeout, PID timer and name timer | Apply OTP-versioned pause distinction and nested resume ownership; timer reconstruction details require the timer audit's cases. |

### MEM-01 — heap, GC, release and hibernation: 16 cases

| Case | Trigger / precondition | Required reference observation or qualification |
|---|---|---|
| MEM-01.C01 | Collection with roots in registers, stack, dictionary, saved arguments, failure state and receive markers | Every live reference remains valid; partially initialized signal slots are not scanned as terms. Rust needs its own admitted-state proof. |
| MEM-01.C02 | Minor collection fits versus old-generation/virtual-binary space exhaustion | Correct minor result or major fallback, including dirty reclassification; no double collection accounting or lost live state. |
| MEM-01.C03 | `fullsweep_after` zero, normal positive and values around stored-field width | Verify major/minor decisions and actual option round-trip; width-boundary behavior remains an unexecuted source concern. |
| MEM-01.C04 | GC disabled/delayed, with zero and positive allocation need | Preserve abandoned heaps and stack roots; represent allocation work and eventual collection rather than losing fragments. |
| MEM-01.C05 | Async GC queued during disabled GC, then enable or death | No premature success; delayed requests become serviceable on enable, while death ordering uses the established signal contract. |
| MEM-01.C06 | Maximum heap disabled, equality boundary, exceedance, kill/log combinations and shared-binary option | Compare the correct accounting domain; distinguish diagnostics from termination and candidate collection space from live bytes. |
| MEM-01.C07 | Minimum heap/virtual-binary settings change | Verify validation/rounding and later allocation effect; do not infer immediate resize or an exact process-memory floor. |
| MEM-01.C08 | Message fragment is consumed, retained by process state, then collected | Transfer off-heap ownership once; freeing the queue node must not invalidate retained payload. |
| MEM-01.C09 | On→off queue conversion with arrivals and GC | Transitional roots remain covered until prior sender activity/conversion completes; mode setter return is not physical conversion completion. |
| MEM-01.C10 | Repeated allocation-mode reversal, conversion yield, then process death | Follow last requested state, retain continuation markers safely and free conversion state on exit. |
| MEM-01.C11 | Dead young versus old-generation binary reference, then final shared owner release | Minor/full-sweep differences do not become leaks; never free another holder's bytes; final physical release happens once. |
| MEM-01.C12 | Magic/native resource reaches zero with outstanding resource-monitor references | Deferred destructor completion remains accounted; callback duration is not automatically bounded and no premature physical free occurs. |
| MEM-01.C13 | `hibernate/3` bad arguments, 255/256/257 argument lists, improper tails or missing resume MFA | Separate early errors from post-stack-reset failure; execute the exact-256 source prediction before choosing an oracle. Accepted hibernation discards old catches/stack and later applies the target. |
| MEM-01.C14 | `hibernate/0` with deep stack/live locals versus `/3` | Preserve `/0` return continuation and live state; do not conflate with `/3` stack reset. |
| MEM-01.C15 | Existing message or arrival between pre/post-compaction fetches, including dirty hibernation | No stranded runnable process; memory optimization must not erase wakeup ownership. |
| MEM-01.C16 | Hibernation below minimum, later GC and memory-limit settings | Verify minimum restoration and peak/limit interactions experimentally; hibernate compaction is a separate path, not a blanket resource-cap promise. |

### STATE-01 — process dictionary and owned metadata: 6 cases

| Case | Trigger / precondition | Required reference observation or qualification |
|---|---|---|
| STATE-01.C01 | First put, replacement and missing get/erase | Preserve creation, old-value and absent-value semantics; a typed Rust representation is still unselected. |
| STATE-01.C02 | Immediate replacement after `get/0` or dictionary inspection | Earlier snapshot values remain unchanged despite internal in-place optimization. |
| STATE-01.C03 | Replacement triggers GC while the old return value is otherwise unreachable | Key/new value/returned old value all survive; result ownership cannot be dropped with the old bucket. |
| STATE-01.C04 | Erase triggers bucket shrink and GC | Preserve the erased return value and remaining entries; no premature root-count reduction. |
| STATE-01.C05 | Dictionary is the remaining owner during hibernation or process death | Live entries survive compaction; death releases owned references with the process, respecting external shared owners. |
| STATE-01.C06 | Large dictionary enumeration/grow/shrink or metadata expansion | Qualify work and allocation cost transitively; no hidden uninterruptible callback or false constant-time claim. |

## Execution still needed, and boundaries still open

The highest-value next reference work is a debug interpreter and x86 JIT matrix for queue conversion plus GC, suspend/resume plus timer pause, hibernation arrival races, and GC requests around death. Numeric setting boundaries and hibernate/max-heap interactions need isolated fixtures before exact promises. The source has both representation-specific optimizations and tests that intentionally rely on internal controls; merely porting their surface assertions would miss their preconditions.

Later multiworker tests need controlled wake/park and claim/reclaim interleavings plus stress, not stress alone. Resource tests must observe last-reference release and callback completion separately. Bream-specific validation must additionally establish public admission, supported Rust layout/lifetime rules and automatically schedulable helper/cleanup work. None is proved by ERTS's moving term heap or native callback conventions.

Unreviewed areas remain explicit: the entire balancing/evacuation policy; OS wake primitives; every architecture's execution/stack layout; full literal/code-purge roots; all binary writer/shrinker paths; every resource destructor; allocator contention/failure behavior; and the full suspend-signal consumer. No claim of universal latency, exhaustive branch coverage, or complete BEAM parity follows from this pass.

Plan integration: these case IDs are recorded in the [expanded build catalogue](../build-plan/process-runtime-expansion.md). All remain planned, with conditional applicability preserved.
