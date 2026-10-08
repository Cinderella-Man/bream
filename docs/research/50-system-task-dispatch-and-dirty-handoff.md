# System-task dispatch and dirty execution handoff

Research date: **2026-10-08**. Reference: Erlang/OTP **OTP-29.1.1**, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. **Source review only: no upstream test, runtime experiment or benchmark was executed.** This deepens the system-work boundary in [report26](26-suspension-system-work-protocol.md), using the existing vocabulary of record retention, lock authority, cleanup obligation and forced termination. Broad native jobs remain conditional research; Bream still requires application execution to stop before `Down`.

The useful new distinction is between **performing runtime work** and **settling its requests**. One collection can satisfy several requests, a postponed request still owns state, and exit can settle a request without performing its original operation. A work bit, execution token and reply obligation are consequently different things. Rust can represent those differences with private owned variants; it cannot remove their scheduling and lifetime protocols.

## 1. A request can cross signal ordering before becoming a task

The request entry copies its correlation value and argument terms into task-owned storage, retaining their off-heap references. Supported public-internal request branches include GC and code inspection; the complete task enum additionally includes literal copying, trace flushing, fixation release, signal-priority service and a test operation. These are runtime operations, not application mailbox callbacks. [Task representation][task-layout]; [request ownership][request-copy].

For GC/code inspection where the caller is also the requester, the entry checks for existing signal work. With signals pending it sends an ordered RPC signal whose callback schedules the task and supplies no separate RPC reply; the task owns the eventual response. Otherwise it can schedule directly. Inherited priority comes from the caller's **user** priority (the requester in this branch); the four-argument entry can name a different requester. Neither a direct task fast path nor replacing a generic request with a typed Rust record may bypass the ordering this branch preserves. [Priority and routing][request-routing]; [ordered callback][request-callback].

Code inspection has an additional scheduling failure condition: dirty application execution. It is redirected to a dirty signal handler selected from the requested priority. If that handler discovers normal execution, it submits the task again; a transition back to dirty execution retries the handoff. Dirty **system** execution is excluded from this redirect because it holds the process main lock. These branches are different ownership destinations for one request, not permission to inspect mutable continuation state without its protection. [Redirect][redirect]; [handler retry][handler].

## 2. Successful state publication does not mean the task list was already linked

`active_sys_enqueue` takes the target's status lock for a real task. It checks the operation-specific failure mask while attempting the state transition, enables system activity and raises effective priority when needed. Only after a successful state transition does it link the task into the selected per-priority circular queue. Consumers also acquire the status lock. Thus task visibility is justified by the combined state/lock protocol, not by treating the scheduler state word as a complete standalone publication flag. [Enqueue][enqueue]; [consumer lock][fetch].

The ordinary path releases the status lock before run-queue insertion; runnable-process tracing keeps it across insertion to preserve diagnostic ordering. Failure before insertion leaves the task with its caller. The upstream `system_task_failed_enqueue` regression describes the historical consequence of violating that rule: rejected tasks remained reachable and could execute after the caller had freed them. The test repeatedly mixes code purges with dirty work; it is source-read evidence, not a reproduced current defect. [Enqueue ownership][enqueue]; [regression][enqueue-test].

A candidate Rust entry should consume a task on accepted ownership transfer and return an owned rejected value on failure, with a private implementation controlling list membership. That API can make ordinary double ownership harder to express. Any unsafe intrusive representation still needs a proof covering the state-before-link interval, rejected transitions and queue retirement; a consuming signature alone supplies none of those facts.

## 3. Task priority, delayed work and completed work differ

`fetch_sys_task` does not always service the highest available task. If the process has active ordinary work, no dirty work, and is neither exiting nor suspended, max-priority user work excludes all lower-priority tasks; high-priority user work excludes normal/low tasks. Otherwise it picks the highest queued class, interleaving low tasks with normal tasks through its own counter. This is task service policy, distinct from the run queue's low/normal process policy. The `otp_16642` test specifies a particular normal/low interleaving with a bound worker and controlled queue contents. It does not establish a global fairness deadline. [Selection][fetch]; [white-box interleaving test][interleave-test].

GC-disabled operations move to a separate delayed queue. That queue still contributes to effective priority when ordinary task queues empty. Re-enabling GC splices delayed tasks to the **front** of each corresponding task queue because they previously reached the front before being postponed, then restores active task flags. It is not a fresh FIFO admission at the tail. The GC-state helper explicitly warns against blindly nesting disable/enable calls; a Rust access-state API could reject invalid nesting or retain an explicit owner, but its exact contract is unselected. [Delayed priority][delayed-priority]; [save][delay-save]; [re-enable][delay-restore].

Normal execution tracks whether a minor or major collection has already run in that invocation, avoiding redundant work while still returning a result to each request. A major collection covers both classes; hibernated processes also have a no-collection success path. A deferred dirty request is prepended to `dirty_sys_tasks`, so admission order is not a universal completion-order promise. Preserve correlation and the supported observable order instead of assuming every internal queue has the same FIFO contract. [GC dispatch][gc-dispatch]; [dirty request list][dirty-save].

The scheduler also avoids task execution while GC is delayed or a BIF is flushing signals, because some tasks need to fetch from the outer signal queue. Running internal work under those temporary conditions would invalidate the helper's protected state even if the task itself appeared well typed. [Scheduler gate][scheduler-gate].

## 4. Runtime completion is work that must be scheduled and owned

| Branch | Actual obligation |
|---|---|
| Ordinary task finishes | Construct the correlated response if the requester is present; always release task-owned off-heap values and task storage. |
| Dirty task finishes | Use a retained requester lookup for this scheduler context, send if present, release that retention and the task. |
| Target exits | GC/code-check/fixation/test tasks settle false, literal-copy tasks settle `ok`, and trace flushing still performs its operation and settles true. A generic “all cancelled” result would erase these distinctions. |
| Signal-priority task has unfinished signal work | Reschedule the same task; fetch from the outer queue only on its first attempt. If exit commits, preserve the required priority elevation through exit service. |

[Reply lifetime][reply]; [exit dispatch][cleanup]; [priority-signal continuation][prio-signal]. The exit path re-enables GC to recover delayed tasks before cleaning task queues. It can yield between cleanup turns; original work completion and exit-time resolution remain different outcomes. [Exit integration][exit-tasks].

Dirty system work runs while holding the main lock and excludes dirty signal-handler service. Its operation order first checks/copies literals, then handles hibernation collection, then ordinary major/minor collection so stronger work can subsume weaker work. The subsequent dirty request loop settles every saved request and only then clears dirty system activity. That loop has **no reduction-budget check**. This is a source-level cost candidate, not a measured latency regression: the actual list length, reply cost and lock contention require a controlled workload. [Dirty operations][dirty-ops]; [completion loop][dirty-complete].

Ordinary task dispatch and exit cleanup have reduction-controlled outer loops, but the completion helper counts reply production rather than bounding every allocation, copy, lock acquisition or destructor. Merely adding an outer budget to the dirty loop would therefore not establish a hard service bound. An incremental Rust completion cursor needs ownership of remaining requests, valid operation results and kill/exit handling at every checkpoint. [Ordinary budget][ordinary-budget]; [reply helper][reply].

## 5. Native return is a separate lifetime transition

Dirty native application calls build a shadow process allocation context, clear their requested dirty work flags and release the main lock around the native function. When it returns, ERTS reacquires the main lock and checks whether the real process is exiting. A live process receives a scheduled normal-scheduler finalizer, exception path or another trapped continuation. An exiting process receives none of those application continuations. Both paths still flush native allocation state and free temporary objects. [Native invocation][native-call]; [return gate][native-return]; [normal finalization][native-finalizer].

Shadow flush updates the allocation top and transfers fragment/off-heap chains into the real process. When both old and new chains are nonempty, it traverses the new chain to find its tail. This gives a concrete Rust representation comparison: an owned segmented result with maintained tail metadata may reduce merge traversal, provided it retains all allocation and final-release obligations. The source already avoids traversal when the destination chain is empty; compare against those fast paths. [Shadow merge][shadow-merge].

The scheduler allows a normal worker to perform forced exit while dirty **application** execution continues, but not while dirty **system** work executes. Exit marks process-body deletion as delayed if dirty execution remains. The final reference release calls `delete_process` for that delayed body. Dirty queue ownership is acquired before queue publication and transferred/released across schedule-out. These mechanisms preserve storage; they do not interrupt the native call. [Exit execution claim][exit-claim]; [dirty queue retention][dirty-ref]; [delayed body][delayed-delete]; [final release][final-delete].

The upstream `dirty_call_while_terminated` fixture waits for `Down` and linked exit, verifies absent liveness/inspection/enumeration, and still observes a retained binary reference until the native call returns. That is direct evidence of the reference's intended test scenario, **not newly executed evidence**. Bream's accepted [ADR0009](../adr/0009-death-and-reclamation-observation.md) deliberately rejects application execution after `Down`. An external host-job facility would need its own explicit lifetime and result-delivery contract; this report does not authorize one or reclassify arbitrary dependency calls as admitted process work. [Reference fixture][native-test].

Dirty CPU reduction accounting measures elapsed time after native execution returns; dirty I/O charges a fixed unit plus virtual reductions. The dirty loop even sets a reduction field it does not use as an execution limit. Accounting these calls does not make them preemptible. [Dirty accounting][dirty-account].

## 6. Eight additional planned branch cases

All cases are **unimplemented and unexecuted**. They refine the existing task/suspension and native-lifetime cases rather than adopting priority APIs, GC or arbitrary native execution into the first milestone.

| Case | Trigger and required qualification | Forbidden result / scope |
|---|---|---|
| **EXEC-08.C23** | Pause task admission around its state transition and list insertion; make its operation-specific failure condition win on another attempt. Track exactly one owner for accepted, rejected and redirected requests. | A rejected task left linked, early consumer access before linking, duplicate reply/free, or losing a task when a competing publisher already activated the target. Use the failed-enqueue regression as an oracle seed. |
| **EXEC-08.C24** | Request GC/code inspection with and without pending signals, including inherited priority and a target changing between normal and dirty application execution. Trace direct admission, ordered RPC and dirty-handler redirection. | Bypassing the required signal order, using effective priority as the inherited user priority, or both the RPC and the task producing the one correlated result. Conditional operation-specific reference mapping. |
| **EXEC-08.C25** | Keep active max/high user work while queuing lower-priority tasks, then make the process inactive/suspended; separately drain normal/low task classes with each queue emptying first. | Treating every task as higher priority than user work, confusing task interleaving with process run-queue policy, or turning a fixture's ratio into a deadline promise. |
| **EXEC-08.C26** | Delay GC-dependent tasks, enqueue newer tasks in the same classes, then re-enable GC or begin exit. Preserve delayed request ownership, priority contribution and front insertion without silently nesting disable/enable ownership. | Orphaned delayed work, reply before prerequisite work is permitted, tail insertion claimed as the reference order, or a task present in both queues. |
| **EXEC-08.C27** | Queue repeated minor/major work and dirty-deferred requests with distinct correlation values; make one operation satisfy several requests and let some requesters disappear before settlement. | Losing individual completion obligations when coalescing work, duplicate computation/replies, leaked task payloads, or inventing FIFO across the dirty pending list. |
| **EXEC-08.C28** | Build a large completion list after one dirty system operation, including expensive correlation payloads and requester contention. For a proposed bounded Rust completion path, pause between replies and inject exit/shutdown. | Dropping unprocessed requests, replaying finished replies, acknowledging complete cleanup early, or claiming the operation's offload/budget bounds reply fanout. This is a planned cost experiment, not an observed ERTS defect. |
| **EXEC-08.C29** | A signal-priority service task exhausts its allowance, reschedules, receives more signals, then races exit or rescheduling rejection. | Re-fetching as though every attempt were the first, losing the retained task, executing application code after exit, or dropping the exit-service elevation prematurely. Distinct from public priority mailbox insertion. |
| **LIFE-05.C21** | At an admitted helper's completion gate, race committed death with result/exception/continuation publication and partial owned allocation transfer. Use dirty-native return only as reference lifetime evidence. | Publishing a continuation after committed death, freeing still-owned partial allocations, or treating record retention as permission to resume. Bream must stop admitted application/helper execution before `Down`; broad host jobs remain unselected. |

## 7. Five Rust-specific performance comparisons

These are **unmeasured hypotheses**, with final backlog numbering assigned in the main performance plan. Each starts from a correct implementation of the supported operations. None requires copying ERTS task kinds or supporting its native ABI.

| Local task | Baseline → treatment | Measure and reject |
|---|---|---|
| **SYS-P01 — Typed owned request records** | Generic boxed request with separately validated operation/argument/result tags → private Rust enum variants containing only that operation's valid typed fields and a compact correlation handle. | Allocation count, record bytes, copied bytes, branch misses, code size and admission throughput. Reject changed signal order, type-confused replies, unsafe escape or dropped ownership on failed enqueue. Compare equivalent admitted operations; do not attribute BEAM term-compatibility work to a Rust requirement. C23/C24. |
| **SYS-P02 — Separate work coalescing from request settlement** | Qualified baseline with existing per-turn work deduplication → explicit pending-work bits plus separately owned completion records that permit additional safe coalescing. | Actual expensive operations per request, queue bytes, reply latency, requester-death retention and mixed-priority tails. Reject loss of correlation, false success, reordered required effects or starvation from merging different classes. Do not claim the source repeats GC for every request. C25–C27. |
| **SYS-P03 — Bounded completion cursor** | Qualified internal operation with one monolithic completion drain → owned cursor over approved result/response records, resumed in bounded runtime turns. | Maximum uninterrupted work, lock-hold time, allocation/copy costs, throughput, peer p99/max delay and stop-to-reclamation tails. Reject unowned residual work, repeated replies, early cleanup acknowledgment or an inner helper whose unbounded work defeats the outer cursor. A cost/latency tradeoff is expected, not a guaranteed throughput gain. C28. |
| **SYS-P04 — Narrow native-result ownership** | Owned fragmented result requiring a scan to join nonempty chains → typed segmented ownership with maintained tail or an equivalent constant-time splice, consumed at a private completion gate. | Merge time versus segment count, extra metadata/cache traffic, kill-path cleanup and physical retention. Reject invalid tail references, skipped final releases or application execution after `Down`. First compare qualified runtime helpers; external host jobs require a separate future decision. LIFE-05.C21. |
| **SYS-P05 — Specialize the normal-priority profile** | Generic priority-class routing compiled into the minimal runtime → a private normal-priority configuration that omits unused queue-class and priority-mapping work while retaining separate control activity and exit cleanup. | State/queue size, dispatch instructions, cycles and binary size against an already optimized compiler baseline. Reject conflating “no priority API” with “no independent control work,” required service starvation or a hidden loss of extension semantics. A later priority-enabled build must retain C25/C29; this does not select a production representation. |

Rust enums can carry different typed data per variant; private construction and exhaustive dispatch can avoid mismatched operation fields. That does not ensure completion: safe Rust permits `mem::forget`, so an exposed destructor alone cannot be the runtime's sole owner of the request or cleanup obligation. Keep those ownership records under runtime control and qualify helper work, allocation and teardown separately. [Rust enum model](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html); [forget contract](https://doc.rust-lang.org/std/mem/fn.forget.html).

## Evidence limits and next experiment

Read: task construction and principal producers, enqueue/fetch/priority transitions, normal and dirty dispatch, delayed queues, reply release, dirty-handler redirection, dirty NIF return and shadow transfer, and exit integration. The cited test sources cover priority inversion, failed admission, normal/low task interleaving, GC-disabled request pressure and dirty-native retained state. **None was run in this pass.** [Priority tests][priority-tests]; [GC-disabled tests][disabled-tests].

Unclosed: the full cross-worker memory-order composition, all task producers and argument lifetimes, complete tracing/fixation helper internals, native halt/reload races, every dirty-BIF return path, allocator failures, complete request-order products and safe Rust implementation. The unbudgeted completion drain and shadow-tail traversal are measurement candidates; no regression or speedup is established.

The first executable follow-up should isolate task ownership with controlled pauses at fail-state validation, task insertion, delayed-queue transfer and result settlement. Model accepted/rejected ownership and request conservation before adding a native job pool. Keep the existing useful-admission and single-worker process gates ahead of conditional dirty/native extensions. No new behavior-preference question or architecture ADR follows from these source facts.

[task-layout]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L457-L497
[request-copy]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L11349-L11407
[request-routing]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L11314-L11504
[request-callback]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L11515-L11574
[redirect]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L11242-L11303
[handler]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erts_dirty_process_signal_handler.erl#L54-L105
[enqueue]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L7025-L7158
[fetch]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10646-L10727
[enqueue-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L5079-L5130
[interleave-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L5206-L5260
[delayed-priority]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10729-L10790
[delay-save]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L11758-L11813
[delay-restore]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L11824-L11930
[gc-dispatch]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10803-L10925
[dirty-save]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L11815-L11820
[scheduler-gate]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10348-L10372
[reply]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10573-L10643
[cleanup]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10999-L11058
[prio-signal]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10930-L10975
[exit-tasks]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14370-L14410
[dirty-ops]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L11094-L11207
[dirty-complete]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L11209-L11238
[ordinary-budget]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10985-L10997
[native-call]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L426-L471
[native-return]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L499-L552
[native-finalizer]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3386-L3435
[shadow-merge]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nfunc_sched.h#L198-L247
[exit-claim]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10084-L10132
[dirty-ref]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L6840-L6906
[delayed-delete]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14748-L14793
[final-delete]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12139-L12150
[native-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/dirty_nif_SUITE.erl#L247-L308
[dirty-account]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_common.c#L150-L208
[priority-tests]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L4929-L5024
[disabled-tests]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L5132-L5199
