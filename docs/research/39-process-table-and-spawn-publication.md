# Process-table allocation, spawn publication and Rust ownership opportunities

Source review, 2026-10-07. Reference: official OTP `29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. **No VM fixture, upstream test, Rust prototype, benchmark or fault injection was executed.** This report deepens allocation/publication branches; [report 27](27-process-lookup-reclamation-protocol.md) owns lookup retention and reclamation, while [reports 13](13-beam-lifecycle-source.md) and [20](20-process-lifecycle-deep-dive.md) cover lifecycle and spawn-request semantics. Rust documentation was checked against official sources; the standard-library pages identify Rust 1.99.0.

The strongest opportunities are explicit construction states, typed non-owning identities, and separating frequently accessed metadata from optional state. These are hypotheses about a native Rust runtime, not an engine selection. Types can prevent some invalid transitions; they do not supply automatic CPU preemption, make arbitrary destructors kill-safe, or prove a concurrent registry correct. Bream still requires stopped application computation and removal from live lookup before `Down`; references may retain runtime records afterward without retaining application life.

## 1. The default allocator consumes free identities, rather than probing occupied process records

`ErtsPTab` separates listing data, frequently modified counters, and read-only configuration into cache-line-sized regions. The pointer table and, in nonlegacy mode, free-ID array are permanently allocated and cache aligned. Initialization rounds requested capacity to a power of two and clamps it to `ERTS_PTAB_MAX_SIZE`, defined as `1 << 27`. This is a capacity-sized table allocation, not allocation of that many `Process` bodies. The bodies are individually allocated later. [Table structure][table-layout], [capacity/allocation][table-init], [maximum constant][table-max], [body allocation][body-alloc].

There are two coordinate systems. Logical ID data is permuted into a physical table index using cache-line masks/shifts; the PID stores the permuted low index bits, allowing lookup to recover the slot with a mask. The free-ID array has a separate permutation for allocation/free cursors. Thus “increment ID” and “visit adjacent process records” are not interchangeable descriptions. The representation deliberately spreads indices across cache lines, but this reading supplies no measured cache benefit. [ID/index transformations][id-map], [free-array cursor transformation][free-map], [array initialization][free-init].

| Branch | Exact transition | Consequence for a Rust design |
|---|---|---|
| Reserve capacity | Increment `count` with acquire ordering while holding the table read lock. If over capacity, CAS-decrement to roll back; if the CAS loses and the newly observed count is within capacity, proceed instead. | The first over-limit observation is not necessarily the returned failure. Account in-progress reservations, not only published pointers. |
| Default allocation | Increment allocation cursor, map it into the free-ID array, exchange the cell with `invalid_data`; retry if the cell was already invalid. | Unique free-token consumption is the reservation mechanism. This loop is not a scan for NULL process pointers. |
| Publish record | Initialize early record fields, initialize its reference count to one, map the chosen ID to its process slot and release-publish the pointer. | Allocation success means the early skeleton is discoverable, not that all fields are ready. |
| Return identity | Release-clear the old process slot; add table size to ID data, mask to finite width and skip the invalid encoding; find a vacant free-ID cell with CAS; only then decrement count. | Identity return, table absence and record destruction are distinct events. A slot generation advances before its identity is made available. |
| Legacy allocation | Increment a global last-ID value, probe pointer slots and CAS NULL to the invalid-element sentinel. After 1,000 failed reservations under the read lock, retry with the table write lock. | A separate deprecated strategy; neither the 1,000 threshold nor lock escalation is a universal time bound. |

[Capacity reservation][reserve], [default consumption/publication][publish], [identity return][release], [legacy probe and publication][legacy], [legacy threshold][legacy-threshold]. The current initialization selects nonlegacy mode by default; `+P legacy` is an explicit alternative. [Mode selection][legacy-default], [option handling][legacy-option].

The default free-ID loops do not contain scheduler reduction checkpoints or a fixed retry count in the inspected function. This does **not** establish starvation or a bug, but rules out citing their short common path as proof of bounded spawn service. A Rust slab/free-list proposal needs its own contention, cancellation and progress argument. The table read lock also coordinates with listing/deletion bookkeeping; replacing this allocator is not merely swapping one integer counter. [Allocation scope][reserve], [deletion/list coordination][release].

## 2. Capacity, wrap and memory accounting need distinct oracles

At maximum table size, initialization increments count and inserts an invalid-element sentinel into one slot; the public maximum/count helpers subtract that reserved slot. Count reporting also clamps transient over-limit reservations. These helpers therefore do not count independently observed fully initialized children. [Maximum-size branch][max-slot], [public count/max][count-view].

**Unresolved maximum-size interaction:** the default free-ID array is initialized earlier with `size` entries. This pass has not reconciled that initialization with exclusion of the maximum-size reserved process slot. The code paths must be checked together in a reduced model or isolated reference harness; a reading of the capacity comment alone is insufficient. No enormous table was allocated and no upstream defect is asserted. [Free-ID initialization][free-init], [reserved slot][max-slot].

ID data has finite width derived from the term width. Returning an ID adds the table capacity and masks it, preserving its physical index while changing its higher incarnation data. Complete PID comparison rejects ordinary stale IDs, but full identity reuse is eventual rather than impossible. Rust's generation field must not silently wrap into a retained old address, queued signal or relationship without an explicit policy. A wider generation is risk reduction, not a proof of eternal uniqueness. [Width][table-max], [reuse arithmetic][release], [full-ID lookup comparison][lookup]. Existing `LIFE-01.C07–C09` own the public reuse/runtime-namespace contract.

There is also a measurement discrepancy worth retaining: `erts_ptab_mem_size` counts the free-ID array using `sizeof(erts_atomic32_t)`, whereas its type, allocation and indexing use `erts_atomic_t`. Their target sizes and alignment must be checked before treating that helper as exact physical bytes. This source observation is not a measured memory difference. Future density comparisons should retain actual allocation sizes, reserved capacity and live-body bytes separately. [Accounting helper][mem-size], [actual array allocation][table-init], [array type][table-layout].

## 3. Local spawn has several publication boundaries

The detailed path is more informative than “allocate, then publish.”

| Phase | What becomes valid / protected | Failure or competing-operation boundary |
|---|---|---|
| Validate and prepare parent | Local parent minor locks are acquired; required spawn/monitor reference is created; MFA/list shape and length are checked. | Recoverable argument/length failures precede child allocation. Reference creation and all subordinate allocation paths are not a complete OOM audit. |
| Allocate body and reserve ID | `erts_alloc_fnf` may return NULL before table reservation. Table reservation can also fail; in that case the allocated body is freed. | Both paths return NULL from `alloc_process` and become `SYSTEM_LIMIT`, with the same “Too many processes” log. OOM of this particular allocation is not distinguished from capacity exhaustion at this boundary. |
| Early table publication | PID, atomic states, run-queue association and initial buffer pointer are initialized; **all process locks start held**; baseline record reference is installed before pointer publication. | Lookup/enumeration can discover the skeleton. They are not authorized to read every field without the required lock/state protocol. |
| Initialize child body | Heap sizing/allocation, argument copying, off-heap ownership, initial continuation, queues, timer fields, dictionary and tracing state are initialized. | Later ordinary allocator calls are not represented as recoverable `alloc_process` failure. Partial-field validity matters to any Rust equivalent. |
| Trace and relationships | Trace paths can release child/parent STATUS and TRACE before installing initial link/monitor endpoints. MAIN and MSGQ remain held through local relationship setup. | Do not claim all locks remain held until the final line, or that trace visibility proves application readiness. |
| Complete local birth | Initial link endpoints and monitor origin/target are installed; an enabled local request success reply is queued before releasing the remaining child locks. | Initial relationship protection precedes application execution; the success reply precedes child-originated signals as traced in report 20. |
| Activate and return | Creator calls `schedule_process`, which sets ACTIVE through state machinery and adds runnable work as needed; spawn returns the PID. | Child may run or exit before the caller resumes. This explicit call is not proof that no external signal could previously request activation. |

[Validation and allocation result][create-front], [early initializer][early-init], [default table publication][publish], [heap/argument initialization][child-heap], [queues and fresh state][child-queues], [trace unlocks][trace-unlocks], [local link/monitor installation][relationships], [reply/unlock][spawn-reply], [creator activation/error label][spawn-end], [schedule-state entry][activate]. Local navigation: [early initializer](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:12159), [creation](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:12436).

The early-discovery branch is real enough to have an upstream regression: `processes_send_infant` enumerates processes and sends to spawning PIDs while spawners use different schedulers. Its subject children use off-heap queues. The fixture is a crash/regression stress test, not a complete publication proof. The scheduler's execution path acquires MAIN and STATUS before using the selected process. These facts support a protected construction phase; they do not justify omitting synchronization in a Rust port. [Infant test setup][infant-test], [enumerate/send/spawn loop][infant-loop], [execution locks][execute-lock].

**Rollback qualification:** after successful table publication, the inspected `erl_create_process` path does not have a general rollback ladder. Its `error:` label only releases parent locks; the visible jumps to it occur before successful `alloc_process`. Heap allocation expands to `erts_alloc`, and local link creation also uses `erts_alloc`; its NULL branch invokes a nonreturning fatal-allocation handler. It would be incorrect to translate this as “BEAM safely rolls back every allocation failure during spawn.” A Rust API choosing fallible later allocations or unwind recovery needs a new staged cleanup protocol. That policy remains unselected. [Heap allocation macro][heap-macro], [fatal versus fallible allocator][alloc-api], [fatal handler][alloc-fatal], [link allocation][link-create], [error label][spawn-end].

## 4. A rare collision path must not be assumed safe

Local spawn-link includes a branch explicitly described as potentially arising when PIDs wrap. It creates a new link, calls lookup-insert, releases the new link if a collision is reported, then accesses the local `lnk->flags` variable. The tree helper returns the existing node on equal key; it does not replace the caller's local pointer. Internal-link release calls `erts_free`. **Taken literally, the collision branch appears to access the released new node.** This is a source-level concern, not a reproduced upstream bug: reachability requires an old same-PID parent relationship to survive until a reused PID is created, and neither that schedule nor its runtime effects was executed. [Spawn collision branch][relationships], [wrapper][link-insert], [equal-key return][tree-collision], [internal release][link-release].

The internal/external distinction matters here: this constructor allocates a standalone `ErtsILink`, initializes flags to zero and sets only its type; it does not create the refcounted two-endpoint `ErtsLinkData` used by external links. The child-side internal link is a second allocation later in the spawn branch. Thus a retained partner reference does not explain away the single internal release in this path. This narrows the source concern; it still does not establish collision reachability. [Internal construction][link-create], [internal versus external layout][link-layout], [type-field update][link-type], [separate child allocation][relationships].

The useful Rust lesson is concrete: an insertion API can return an owned enum distinguishing `Inserted` from `Existing`, with the rejected allocation moved into one cleanup path. Subsequent code must use the retained entry, not an alias to the discarded candidate. Such an API can prevent this class of local use-after-release in safe code; it cannot decide whether full-ID reuse should merge, replace or reject a still-present relationship. That semantic case remains a model/reference gate.

The upstream `no_pid_wrap` test churns for three minutes, treating observed reuse as failure on 64-bit and permitted on 32-bit. That is finite empirical coverage encoded in a test, not proof that the identity space never wraps. A separate debug helper can rewrite upcoming free IDs under a table write lock and asserts free-token/NULL-slot/count consistency. Neither was run here. [Finite wrap test][wrap-test], [debug consistency and ID control][debug-id].

## 5. Where Rust can provide a specific advantage

These are proposed internal designs; names below are explanatory, not public API commitments.

| Opportunity | What Rust could enforce or remove | What remains dynamic |
|---|---|---|
| Typed identity | Private `ProcessId` fields distinguish identity from a pointer; `Address<Message>` can carry a zero-sized protocol marker and reject wrong-protocol calls in safe code. Explicit occupied/vacant insertion results can move ownership through collision handling. | Runtime instance, slot generation, liveness, serialization validation and compatible erased-address conversion. A marker is not runtime branding and is not an access-control boundary against unsafe host code. |
| Staged publication | Consuming internal `Reserved → Initialized → RelationshipsReady` values can make the ordinary safe enqueue function unavailable before setup. Admitted initial state can be moved into its owner instead of copying a generic term graph. | Parent death, cancellation, allocation failure, shared-registry atomics and initialization observed through other handles. A linear local builder does not serialize other workers. Large moves are not guaranteed byte-copy-free. |
| Guarded access | A short-lived borrow tied to a registry/lock guard can make references unable to outlive the protection that authorized them. Scheduler-owned state can use exclusive access when its owner is actually exclusive. | Guard construction, reader registration, memory ordering, retained queues and body-versus-record lifetime. A lookup guard is not a lease on process life and must not allow application-state access after stop. |
| Workload-specific representation | A typed process frame can omit unused VM fields; hot lifecycle/scheduling fields can be separated from optional trace, dictionary or inspection metadata. | Actual padding, code size, first-use allocation cost and independent metadata ownership. Heterogeneous process frames still need safe type erasure or separate typed storage. |

Rust supports private fields/items, lifetime-checked references and zero-sized `PhantomData`; default struct layout does not promise declaration-order offsets. Those language facts permit experiments, not the claim that any proposed wrapper is automatically sound or cost-free. Verify marker variance and `Send`/`Sync` behavior against the intended ownership surface. [Privacy][rust-privacy], [borrowing][rust-borrow], [marker semantics/layout][rust-phantom], [layout guarantees][rust-layout].

ERTS already pursues hot/cold separation: the `Process` declaration groups frequent BEAMASM fields for shorter addressing and paired stores, and puts rarely used data behind `psd`. Initial PSD installation allocates and initializes a candidate, CAS-publishes it, and frees the losing candidate. Rust's advantage would be enforcing the ownership of those alternatives and specializing an appropriate supported profile, not discovering a technique absent from BEAM. [Process layout][process-layout], [cold pointer][process-cold], [PSD publication][psd-publish].

Two boundaries prevent attractive but invalid shortcuts:

- `Arc` retains its allocation/value through strong ownership; successful `Weak::upgrade` establishes allocation availability, not Bream process liveness. An internal `Arc<Record>` may be a legitimate retention baseline, but address counts must not control application termination, nor force application resources to remain until every address disappears. [Arc semantics][rust-arc].
- `MaybeUninit` does not track which fields were initialized and dropping it does not clean up those fields. A builder needs initialized-resource accounting and cannot expose a partially initialized `&Process`. Safe Rust also permits forgetting a value, so a public guard's destructor cannot be the sole memory-safety assumption. Runtime-owned construction/control records should remain in the admitted runtime protocol; no arbitrary user destructor is introduced into forced cleanup. [Initialization requirements][rust-uninit], [destructor non-guarantee][rust-forget], [invalid-reference rules][rust-ub].

## 6. Five falsifiable Rust experiments

All require relevant LIFE-01, LANG-01/02/03 and EXEC-03/04 ownership cases before performance conclusions; multiworker treatments additionally require EXEC-07. Preserve automatic scheduling and stop-before-`Down`. Use the same admitted work and record compiler, target, allocator, worker count, instrumentation and table occupancy. No task authorizes a new mailbox quota or uninterruptible native escape.

| Local task | Baseline → treatment | Measure and accept/reject |
|---|---|---|
| SPAWN-R01 — Typed identity and entry results | Runtime-tagged generic addresses/boolean insertion result → private typed addresses and ownership-bearing occupied/vacant results, retaining the same identity checks. | Compile-fail wrong-protocol/invalid-transition examples plus code size, address size, dispatch branches and collision allocations. Accept eliminated checks only where types actually prove them; reject if runtime-instance/generation checks disappear or code expansion outweighs benefit. |
| SPAWN-R02 — Construct before runnable publication | One mutable construction record with flag checks → consuming staged builder, prevalidated/preallocated relationship endpoints and moved admitted input. | Count allocations/copies, spawn latency, fault-injection cleanup, concurrent infant sends, and parent/child death at every boundary. Accept fewer checks/copies with identical lifecycle observations; reject successful publication followed by partial rollback, or a new long uninterruptible construction segment. |
| SPAWN-R03 — Stable chunked process storage | Individually boxed record plus protected free-token pool → fixed-location chunked slots with explicit generations and the same safe lookup protocol. | Occupancy sweeps, churn, allocation count, page/cache behavior, reserved versus live bytes, retired-slot retention and reuse contention. Reject moving storage under outstanding references, generation aliasing, stranded capacity or retry/scan work hidden from peer/kill measurements. |
| SPAWN-R04 — Separate hot and optional metadata | Fully provisioned common record → small fixed header plus lazily allocated cold metadata and separately typed process body. | Idle density, send/kill lookup cost, first trace/inspection/dictionary activation, allocation failure and cold-state death races. Accept a reproducible footprint/service improvement; reject if indirection or first-use stalls dominate, or optional metadata owns the body accidentally. Compare against BEAM's existing PSD strategy fairly. |
| SPAWN-R05 — Borrow for immediate access, retain for deferred work | Internal retained-record reference on each lookup → scoped guard-bound borrow for immediate operations, retaining explicit owned protection when work escapes the guard. | Atomic reference operations, lookup/send latency, stalled-reader retention, shutdown behavior and compile-fail escaped-borrow examples. Accept reduced churn only with a proved reclamation/aliasing protocol; reject guard leakage causing unsafe reuse or a host-held guard blocking required kill-to-stop progress. No public `Arc` application-lifetime semantics. |

Every run must report throughput alongside peer progress, spawn tail, kill-to-stop, stop-to-`Down`, final cleanup and retained bytes. Improving spawn throughput by doing larger unaccounted argument/metadata work is not an accepted win. Reduced-width identity and fault-injection fixtures are correctness experiments, not performance numbers. Compile-fail success likewise does not substitute for concurrent execution/model coverage.

## 7. Eight proposed behavior refinements

At reservation, `LIFE-01` ended at C10. The following **LIFE-01.C11–C18** are new planned cases; no shared plan/index was edited. They refine initial identity/construction obligations, with explicitly conditional storage/multiworker variants.

| Case | Trigger / branch | Required result or unresolved evidence |
|---|---|---|
| LIFE-01.C11 | Concurrent capacity reservation crosses the limit; a competing reservation rolls back or an exiting record returns its identity. | Exactly one outcome per reservation, coherent free-token/count accounting, no application execution from an incompletely initialized child. Exercise both CAS rollback success and retry-then-proceed. Do not equate initial over-limit observation with final failure. |
| LIFE-01.C12 | Default free-ID consumption encounters holes, stalled return publication and cursor wrap; separately enable a probing allocator. | No duplicate ownership or overwrite of a live slot; free tokens match vacant eligible slots after quiescence. Measure retry work and require a progress argument; legacy lock escalation is conditional reference coverage, not an initial Rust feature. |
| LIFE-01.C13 | Capacity rounds upward, reaches absolute maximum, or identity arithmetic reaches invalid/full-wrap encoding. | Reconcile reserved sentinel with free pool and public count, reject invalid IDs, preserve index/generation mapping. Maximum-size interaction is source-unresolved; use a reduced model before a large reference run. Extends C08 without silently selecting Rust width/wrap policy. |
| LIFE-01.C14 | PID is enumerated while creator holds construction locks; send, monitor, inspect or kill races trace-lock release and remaining initialization. | No uninitialized access, lost accepted input or application execution before initial relationship protection. Trace/enumeration visibility is not readiness; collect allowed race outcomes per operation. Full cross-operation oracle remains unexecuted. |
| LIFE-01.C15 | Body allocation fails before reservation; table full after body allocation; later heap/endpoint/optional-metadata initialization fails or unwinds. | Distinguish reference's recoverable SYSTEM_LIMIT paths from fatal allocator paths. For each supported Rust recovery branch, release only initialized owners and reservations exactly once; no orphan endpoint. Public error/input-return/panic policy remains unselected. |
| LIFE-01.C16 | Reused full PID collides with an existing parent link during spawn-link, including priority metadata if supported. | Existing versus rejected allocation ownership is explicit; forbid post-release access and duplicate priority accounting. Determine merge/replace/reject semantics via reduced model/reference work; source concern is not a demonstrated upstream failure. |
| LIFE-01.C17 | Typed address is erased/restored, copied across runtime instances or retained through slot reuse; a record borrow attempts to escape its guard. | Compile-time protocol separation plus runtime namespace/incarnation validation; no safe escaped reference. Retained record does not retain application life; dropped/forgotten addresses cannot change stop semantics. |
| LIFE-01.C18 | Chunked slab grows or cold metadata first publishes while another operation reads, kills or retires the record; initialization loses its CAS. | Stable valid storage under protection, single owner of winning metadata, one cleanup of losing candidates, tracked partial initialization. Compare actual allocation sizes with reporting; representation-specific subcases apply only if selected. |

**Coverage boundary:** traced table initialization, capacity reservation, default and legacy ID allocation, return/reuse arithmetic, early process publication, local initialization/relationship ordering, allocation-failure classification and selected cold-metadata publication. Sampled the infant-send and finite-wrap tests. Unreviewed or unproved: complete process enumeration algorithm, weak-memory proof, all lookup consumers, maximum-size/default-pool consistency, actual collision reachability, allocator internals, every trace callback, remote spawn rollback, Rust admission/kill safety and any performance result. These remain explicit tasks rather than claims that source coverage establishes every interleaving.

[table-layout]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.h#L114-L169
[table-init]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L377-L417
[table-max]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.h#L171-L200
[body-alloc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12179-L12218
[id-map]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.h#L288-L341
[free-map]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L337-L345
[free-init]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L432-L462
[reserve]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L500-L528
[publish]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L530-L558
[release]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L675-L734
[legacy]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L562-L640
[legacy-threshold]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L38-L42
[legacy-default]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_init.c#L1301-L1309
[legacy-option]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_init.c#L1864-L1880
[max-slot]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L472-L488
[count-view]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.h#L266-L284
[lookup]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.h#L1135-L1156
[mem-size]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L349-L354
[create-front]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12465-L12540
[early-init]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12159-L12174
[child-heap]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12543-L12652
[child-queues]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12722-L12749
[trace-unlocks]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12810-L12902
[relationships]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12908-L12945
[spawn-reply]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12947-L13007
[spawn-end]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13115-L13142
[activate]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L7005-L7017
[infant-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L4558-L4592
[infant-loop]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L4613-L4668
[execute-lock]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10185-L10206
[heap-macro]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.h#L86-L95
[alloc-api]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc.h#L248-L301
[alloc-fatal]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc.c#L2099-L2109
[link-create]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.c#L1466-L1493
[link-insert]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.c#L1282-L1287
[tree-collision]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_rbtree.h#L1071-L1089
[link-release]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.h#L2462-L2482
[link-layout]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.h#L1641-L1660
[link-type]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.h#L468-L480
[wrap-test]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L5955-L6001
[debug-id]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L1329-L1408
[process-layout]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.h#L1043-L1079
[process-cold]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.h#L1133-L1175
[psd-publish]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L1551-L1572
[rust-privacy]: https://doc.rust-lang.org/reference/visibility-and-privacy.html
[rust-borrow]: https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html
[rust-phantom]: https://doc.rust-lang.org/std/marker/struct.PhantomData.html
[rust-layout]: https://doc.rust-lang.org/reference/type-layout.html
[rust-arc]: https://doc.rust-lang.org/std/sync/struct.Arc.html
[rust-uninit]: https://doc.rust-lang.org/std/mem/union.MaybeUninit.html
[rust-forget]: https://doc.rust-lang.org/std/mem/fn.forget.html
[rust-ub]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html

The proposed cases are integrated in the [build plan](../build-plan.md); the report-local experiments feed the stable [performance task backlog](../performance-plan.md). The [Rust opportunity map](../rust-opportunities.md) compares these mechanisms without selecting a production representation.
