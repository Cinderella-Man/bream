# Native resource callbacks and owner lifetime

Inspected **2026-10-07**, Erlang/OTP **OTP-29.1.1**, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. **Source review only: no test, benchmark, native callback experiment or Bream implementation was executed.** This extends [report22](22-process-io-native-boundary.md) and the storage lifetime distinctions in [report40](40-allocator-ownership-and-rust-runtime-storage.md). It covers the resource integration boundary, not NIF API compatibility, arbitrary native dependencies or bundled library algorithms.

The main result is that there is no single “resource owner is dead” transition. Strong resource owners, monitor protocol references, in-flight callbacks, event registrations and callback code have different lifetimes. Rust types can remove some dynamic checks and make ownership transfers explicit, but they cannot erase a callback or poller that still uses the resource.

## 1. A resource is not automatically owned by its creating process

A NIF resource is a native allocation exposed through opaque Erlang terms. Terms can move between processes; explicit native `keep/release` ownership can outlive those terms. Destruction requires the strong owners to disappear, not merely the death of the process that created or first used the object. The resource type's `owner` field denotes the implementing NIF library, **not an Erlang process**. Mutable resource data requires the library's synchronization. [Public resource ownership][resource-doc]; [native concurrency][concurrency-doc]; [type/resource structures][structures].

For this report, an “owner-process monitor” means a resource monitors a process whose death its library chooses to handle. BEAM does not infer that relationship from allocation. Bream's process-owned resources will need an explicit supported mapping rather than inheriting generic NIF sharing unnoticed.

| Lifetime | Pinned representation and responsibility |
|---|---|
| Strong resource ownership | Magic binary reference count. Allocation creates a native reference; keep/release adjust it; resource terms and resource-backed bitstrings carry further ownership. [Creation/strong references][strong]. |
| Outstanding resource monitor protocol | `ErtsResourceMonitors.refc`, protected by its mutex, with a DYING flag in the same integer. Keeps resource storage valid while monitor endpoints/signals settle, but does not prevent the zero-strong-reference destruction protocol from starting. [Counter][rmon]; [destructor coordination][dtor]. |
| Monitor record endpoints | Each resource monitor contains origin and target endpoints, with its own atomic count initially two. Origin belongs to the resource tree; target belongs to the process's local-target list. [Representation][monitor-layout]; [allocation/refcount][monitor-create]; [endpoint release][monitor-release]. |
| Running `down` callback | Claims a temporary strong binary hold only if the strong count is not already zero; releases it after the callback. [Callback claim][fire]. |
| Registered OS event | Registration retains the containing resource; the normal stop path releases that hold after the stop callback returns. Several event relations can retain one resource. [Registration][select-keep]; [stop release][select-direct]; [scheduled release][select-scheduled]; [public relation][select-doc]. |
| Callback implementation | Resource-type/library reference bookkeeping prevents unloading callback code while it is needed. This is independent of process identity. [Library ownership][type-owner]; [purge][unload]. |

These are accounting roles, not necessarily six independent allocation objects. For example, monitor metadata is appended to the resource allocation only when its type has a `down` callback. [Resource layout branch][resource-create].

## 2. Monitor registration and cancellation have claim boundaries

`enif_monitor_process` requires a live resource reference and a type with a `down` callback. No callback capability returns a negative result; an undefined/dead target returns positive; successful publication returns zero. It allocates a fresh internal reference and monitor record, inserts the origin under the resource mutex and increments the monitor-protocol count before sending the target endpoint. Failed publication removes the origin, rolls back the count and releases both monitor endpoints. A successful return does not assert the target is still alive then. [Public result contract][monitor-doc]; [registration and rollback][monitor].

Ordinary reception inserts the target endpoint into `ERTS_P_LT_MONITORS`. A monitor signal arriving during target exit is routed through the exit monitor handler instead. Thus success followed immediately by target death is not the same branch as failure to publish to a nonexistent target. There is no visible callback-after-API-return barrier: publication precedes writing the optional caller output monitor identifier. **Early callback relative to the registering caller's return is a source-permitted interleaving to probe, not an executed result here.** Callers cannot safely initialize callback-shared state only after registration returns. [Target insertion][monitor-receive]; [late signal][monitor-late]; [publish/output order][monitor].

Repeated monitoring of the same target produces separate references. The resource's origin tree is keyed by those references; the target can contain multiple resource monitor entries. A resource callback receives PID and monitor identity, **not an exit reason argument**. It is not itself an ordinary mailbox `DOWN`, although library code may send a message from it. [Endpoint layout][monitor-layout]; [down callback signature][callback-doc]; [independent-monitor test][test-demonitor].

`enif_demonitor_process` converts the identifier, looks it up in that resource's tree, removes it under the mutex, and then sends the origin endpoint as a demonitor signal. Return zero means it won removal; nonzero includes wrong resource, repeated cancellation, already fired and concurrently claimed monitors. Successful removal suppresses a future `down` invocation for **that monitor**, because the firing path tests origin membership under the same mutex. It is not proof that the target-side record is already reclaimed, nor a join with other callbacks. A failed removal cannot be interpreted as callback completion. [Cancellation][demonitor]; [public race outcomes][demonitor-doc]; [claim test][fire].

Target-side demonitor handling removes the local-target entry and calls `erts_nif_demonitored`, which decrements the resource's monitor-protocol count. If the resource is DYING and this was the last outstanding protocol reference, it re-enters the resource destruction prologue. When target exit already handled the target endpoint, the later demonitor signal instead releases its remaining endpoint. The two counts must not be merged: endpoint memory and resource lifetime finish through different releases. [Signal handling][demonitor-receive]; [acknowledged monitor release][demonitored]; [late demonitor][monitor-late].

## 3. Target death races last strong release

`erts_fire_nif_monitor` resolves the resource from the target endpoint and locks its monitor state. It decrements the outstanding monitor count, observes DYING and tests origin membership. An active origin is removed. Only then does it try to acquire a strong binary reference **unless the count is zero**. User `down` runs outside the resource mutex and only with that successful hold. [Full firing path][fire].

| Interleaving branch | Required source outcome |
|---|---|
| Origin already canceled, resource still alive | No user `down` for that monitor; release target endpoint. |
| Resource already DYING | No user `down`; the last protocol reference can restart deferred destruction. |
| Origin active, strong count already zero but DYING not yet set | Remove the origin but do not resurrect the resource or invoke `down`; existing destruction work owns progress. |
| Origin active and strong hold acquired | Invoke `down`; resource storage survives its body even if other strong owners disappear; release callback hold afterward. |

No unique race winner is promised. In particular, “the monitored process exited” does not imply a callback when the resource itself loses its last strong owner in the race. Conversely, a callback that already acquired its hold must not be interrupted by freeing its resource. These statements concern valid ownership use; concurrent use of an unretained raw resource pointer is outside the API contract. [Firing/refcount branches][fire]; [resource ownership][resource-doc].

The last strong release invokes the magic-binary destructor prologue, which schedules auxiliary work in a known lock context and declines immediate raw free. `run_resource_dtor` marks monitor state DYING once, detaches its origin tree under the mutex, sends demonitor signals for the detached entries, and returns while protocol references remain. When none remain, it destroys the monitor mutex, calls the user destructor, releases the resource type and frees the magic binary. The public API does not promise when or on which thread the destructor runs; this revision's prologue is specifically deferred. [Prologue/destructor][dtor]; [demonitor walk][dtor-walk]; [release contract][release-doc].

This establishes **destructor last**, not “all callbacks are serialized.” The callback hold excludes destruction while `down` runs, and event holds exclude destruction until stop returns. But separate `down` invocations and other nonfinal callbacks can still need application synchronization. The public destructor contract allows only final access to resource user data: it is not a resurrection hook for publishing another handle or installing a new monitor. [Last-callback rule][callback-doc]; [mutable-resource synchronization][concurrency-doc].

## 4. Process DOWN is not native-resource cleanup completion

The normal exit continuation removes the live PID before relationship processing. Its ordinary monitor tree is processed before the local-target monitor list containing resource monitors. The latter calls `erts_fire_nif_monitor` synchronously after dropping the exiting process's MAIN lock. Therefore an ordinary process monitor notification can already be delivered while resource `down`, stop coordination or final destruction still has work. This is an implementation-order observation for the inspected exit path, **not a global ordering promise for messages emitted by different callbacks**. [Identity removal][identity-remove]; [phase order][exit-order]; [native monitor dispatch][exit-fire].

The list walk can yield between operations, but a native callback body is called synchronously. The resource destructor's detached monitor-tree walk also has no yielding continuation in this function. Placing the destructor in auxiliary work solves its execution-context problem; it does not bound callback duration, the monitor walk, environment cleanup or resource-specific teardown. [Exit invocation][exit-fire]; [destructor walk][dtor].

Bream's accepted `Down` means admitted application execution stopped and live identity disappeared; tracked runtime reclamation may follow. This source behavior reinforces that separation. It does **not** authorize arbitrary native callback code to continue as untracked application work after `Down`, or establish that Rust `Drop` can be forcibly interrupted safely.

## 5. Stop and callback reentrancy need an explicit contract

The stop wrapper constructs a callback environment and directly calls the resource's stop function. Direct stop is invoked after releasing the FD mutex, and the retained registration reference is released afterward. Scheduled stop similarly detaches the state, unlocks and calls stop before releasing ownership. Thus a caller of `enif_select(...STOP...)` must be prepared for its callback to run **before that call returns**. A `down` callback which requests direct stop can therefore cause nested callback work. This does not license arbitrary recursive NIF operations or prove they cannot deadlock on library-owned locks. [Wrapper][stop-wrapper]; [direct call][select-direct]; [scheduled call][select-scheduled]; [stop call modes][callback-doc].

Callback environments are temporary pseudo-process environments, not the monitored process's environment or application continuation. Setup clears a pseudo-process structure; post-callback clearing releases temporary objects, off-heap references and heap fragments. Terms stored beyond callback lifetime need a valid transfer/owning environment. The upstream `monitor_resource_down` test explicitly exercises `enif_whereis_pid/port` inside a down callback to catch a lock-order regression; that example is not a blanket guarantee for every API or lock combination. [Environment contract][env-doc]; [setup/clear][env]; [lookup regression body][test-down].

The event-resource relation is dissolved only when stop returns. Receiving or canceling all readiness messages does not discharge it, and process death alone does not break it. This is why a resource monitor is useful to initiate stop when a user process dies. Reactor/backend and descriptor-reuse details are handled separately from this resource lifetime report. [Relation contract][select-doc]; [report22](22-process-io-native-boundary.md).

One sampled source discrepancy remains **unconfirmed**: the delayed STOP branch for `state->type == NONE` calls `enif_keep_resource(resource)`, while the normal keep and stop-release paths pass `resource->data`. Ordinary unknown-FD STOP fast-tracks earlier on the continuous-FD path; valid-call reachability of the discrepant branch has not been established in this review or the parallel backend trace. This is a named reachability/instrumentation gap, **not a reproduced bug or an expected behavioral oracle**. [Exceptional branch][stop-anomaly]; [early direct stop][stop-fast]; [normal hold][select-keep].

## 6. Callback code has its own lifetime

Resource types with down/stop/dynamic-call callbacks but no user destructor receive an internal no-op destructor, allowing the callback-presence check to retain native library code. Purging the Erlang module detaches its resource types and drops the module's ownership; surviving resources can still retain callback code. A regression test explicitly checks down after purge both with and without a destructor. [Callback capability normalization][dummy-dtor]; [unload bookkeeping][unload]; [purge test][test-purge].

Takeover additionally installs read-locking callback wrappers, updates the real callback table under the corresponding write-lock regime, then removes the wrappers during cleanup. This is more than replacing a function pointer. A statically linked Rust runtime can avoid this machinery **if dynamic resource-code replacement is excluded**; that choice does not remove ordinary process/resource concurrency. [Wrappers][takeover-calls]; [table transition][takeover]; [final type release][type-owner]. Full loading/upgrade coordination is outside this bounded trace.

## 7. Rust can simplify representation, not erase in-flight use

These are proposals for later qualified resource support, not permission to pass arbitrary native objects into current processes.

- **Typed resource and monitor tokens** can rule out some wrong-type/wrong-resource operations before runtime and avoid converting an Erlang reference term merely to find an internal monitor record. A token still needs runtime generation/lifetime protection and explicit cancel-versus-claimed state; a Rust move cannot recall a callback already admitted on another worker. The BEAM comparison is its dynamic type test and extended monitor record, not a claim that those checks are free to remove from an untyped public API. [Dynamic type check][get-resource]; [monitor record][monitor-create].
- **An owned stop job** could receive the unique resource or a dedicated registration hold and return it only after the backend's safe-close boundary. `OwnedFd` closes its descriptor on drop, and `BorrowedFd` expresses a borrowing lifetime; neither automatically tracks an OS poller that retained a raw handle. Dropping the process's field is safe only when the runtime's outstanding-use protocol permits the close. [OwnedFd][rust-owned]; [BorrowedFd][rust-borrowed].
- **Typed runtime events instead of extensible callbacks** could eliminate temporary term environments, callback dispatch and application reentrancy for approved operations. That restricts the extension surface and requires qualified runtime handlers; it cannot be sold as compatibility with arbitrary user callbacks. Callback bodies, environment cleanup and final native release remain separate costs in the baseline. [Environment machinery][env]; [stop wrapper][stop-wrapper].
- **Ownership transfer can replace some shared reference operations** when the compiler/API proves one live owner and queues transfer it completely. When resource terms, event relations or callbacks overlap, shared holds remain necessary. `Arc` supplies reference-counted shared ownership; `Send` describes thread transfer, not cancellation, bounded callbacks or safe destruction of external state. [Arc][rust-arc]; [Send][rust-send]. Resource ownership and cleanup representation must remain closed under the admitted operations.

## 8. Eight additional cases

These extend IO-01.C01–C08 and are **planned, unimplemented and unexecuted**. Native-resource expansion remains conditional. A Rust-specific alternative must preserve the supported observable contract without claiming every NIF capability.

| Case | Trigger and required/forbidden observations |
|---|---|
| **IO-01.C09** | Register without down capability, with undefined/dead target, during target exit and twice against one target. Check failure rollback, independent identities and publication-versus-return ordering; forbid callback state that is uninitialized when publication becomes visible. Sources: [registration][monitor], [target late handling][monitor-late]. Early callback timing remains an unexecuted probe. |
| **IO-01.C10** | Cancel using wrong resource, repeated identifier, or while the firing path claims origin. Zero suppresses that monitor's future callback but need not reclaim target-side state immediately; nonzero is not callback completion. Forbid treating failed cancellation as a join or successful cancellation as cancellation of another monitor. [Cancel][demonitor]; [race contract][demonitor-doc]. |
| **IO-01.C11** | Race last strong resource release with origin lookup/conditional callback hold and DYING publication. Accept the source branches: callback with a valid hold, or suppression without resurrection. Forbid destructor overlap with a running held callback, double endpoint release or stuck monitor counts. [Fire][fire]; [dtor][dtor]. |
| **IO-01.C12** | The monitored process holds the last Erlang resource term; compare another process/native owner retaining a handle. Record PID removal, ordinary process DOWN, resource callback and final destruction separately. Forbid assuming creator death uniquely owns or destroys shared resource storage, or using process DOWN as a cleanup barrier. [Strong ownership][strong]; [exit order][exit-order]; [self-monitor test][test-self]. |
| **IO-01.C13** | Multiple monitored targets exit concurrently; callback cancels another monitor or requests direct stop; callback allocates/sends terms. Qualify mutable-state synchronization and nested callbacks without holding incompatible locks. Forbid escaping temporary environments, assuming per-resource callback serialization, or arbitrary callback admission based solely on `Send`. [Fire][fire]; [direct stop][select-direct]; [environment][env-doc]. |
| **IO-01.C14** | One resource has multiple event relations and native/term owners; stop one relation directly and another asynchronously while owners disappear. Destructor is last; each registration hold survives its stop return. Forbid closing/reusing an event through ordinary field Drop before safe stop, or attempting resource resurrection from the destructor. [Relation][select-doc]; [stop release][select-scheduled]; [dtor contract][callback-doc]. |
| **IO-01.C15** | Last strong release detaches a large monitor tree; interleave target death, demonitor acknowledgments, late target signals and final protocol-reference release. Retain resource storage until settlement; measure the unsliced walk and nested cleanup separately. Forbid lost cleanup ownership, duplicate destructor scheduling effects or claiming local count accounting is a latency bound. [Dtor walk][dtor]; [ack][demonitored]; [late signal][monitor-late]. |
| **IO-01.C16** | Purge/replace resource callback code while resources remain, including a down-only type with no user destructor; alternatively exercise an explicitly static Rust resource implementation. Forbid callback into unloaded code or inferring no callbacks from an absent user destructor. Dynamic takeover remains unsupported unless independently qualified. [Normalization][dummy-dtor]; [purge][unload]; [regression][test-purge]. |

## 9. Five unmeasured performance tasks

Each comparison needs correctness/race fixtures first, identical supported semantics and source/build manifests. Measure through final tracked cleanup, not just callback enqueue. Also report peer-ready delay, kill-to-stop, stop-to-cleanup, resource/monitor allocations and retained bytes.

| Local task | Baseline → treatment | Measurements and constraint |
|---|---|---|
| **PERF-44.T01: typed monitor record** | Generic resource kind plus dynamic reference representation → typed resource association and fixed-layout monitor ID/record. | Registration/cancel/claim cost, allocations, lookup cache misses and stale-ID handling. Preserve C09/C10/C11; Rust typing can reject invalid kinds but cannot remove runtime claim or endpoint lifetime coordination. |
| **PERF-44.T02: inline common ownership edge** | Tree/allocation for every monitor → inline first monitor with an overflow structure when more are supported. | Single-owner and repeated/many-target workloads, allocation count, promotion cost and teardown. C09/C15 must preserve separate monitor identities; no silent deduplication or restriction on a claimed supported multi-monitor interface. |
| **PERF-44.T03: owned stop transfer** | Repeated shared holds/general callback job → move a qualified stop capability and its ownership into runtime cleanup. | Atomic reference operations, queue hops, direct/deferred stop cost, registration churn and retained lifetime. C11–C14; preserve backend outstanding-use protection. If ownership overlaps, retain shared holds rather than optimizing them away. |
| **PERF-44.T04: typed lifecycle events** | Generic callback plus pseudo-process/term environment → runtime event enum and statically selected qualified handler. | Event construction, temporary bytes, dispatch/code-size tradeoff, concurrent target death and reentrancy cost. Preserve supported notifications and safe cleanup; callback extensibility is not equivalent and must remain an explicit boundary. |
| **PERF-44.T05: resumable resource teardown** | Entire detached monitor/resource ledger walk → explicit runtime cleanup cursor with separately accounted final-release jobs. | Maximum observed uninterrupted segment, peer progress, monitor-settlement throughput and cleanup retention after the last user process stops. C15 and shutdown; no arbitrary user destructor moved into an allegedly bounded step. Static callback lifetime can omit takeover bookkeeping only under the exclusion in C16. |

Selected upstream evidence was **read, not run**: monitor variants test ordinary down, native-thread last release, self-monitoring with the final Erlang term, a release/exit race, repeated independent monitors and code purge. The stress helper keeps an explicit resource hold while registering/canceling, tracks pending callback state and asserts destruction does not leave a pending callback. Its native pointer/locking idioms are not a Rust safety template. [Basic/resource-death tests][test-basic]; [self/release race][test-self]; [demonitor test][test-demonitor]; [stress state transitions][test-stress]; [stress destruction][test-stress-down]. An expected winner in one upstream fixture is not the only permitted outcome of the general race.

Traced: strong resource ownership, monitor setup/rollback/cancellation, down claim/suppression, destructor deferral, target-side release and exit integration, normal stop hold/release, temporary callback environments and principal code-retention branches. Sampled: takeover synchronization and upstream scenarios. Unreviewed: a complete weak-memory/reentrancy proof, every resource API, all platform stop transitions, dynamic-call implementations, arbitrary native library callbacks, complete code upgrade/shutdown protocol and the exceptional stop-pointer branch's reachability. No speedup, callback duration bound, complete native integration or production resource design is claimed.

Local navigation: [down claim](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_nif.c:3059), [destruction](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_nif.c:2974), [registration/cancel](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_nif.c:3933), [exit phase order](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14560).

[resource-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L199-L223
[concurrency-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L281-L287
[structures]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/global.h#L73-L104
[strong]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3143-L3222
[rmon]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L2893-L2938
[dtor]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L2961-L3029
[monitor-layout]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.h#L215-L231
[monitor-create]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.c#L920-L1039
[monitor-release]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.h#L1503-L1530
[fire]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3059-L3118
[select-keep]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/common/erl_check_io.c#L1440-L1463
[select-direct]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/common/erl_check_io.c#L1485-L1538
[select-scheduled]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/common/erl_check_io.c#L2182-L2247
[select-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L3577-L3608
[type-owner]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L2620-L2629
[unload]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L5334-L5372
[resource-create]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3121-L3161
[monitor-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L2994-L3028
[monitor]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3933-L3989
[monitor-receive]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6544-L6561
[monitor-late]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7347-L7360
[callback-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L738-L772
[demonitor]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3998-L4030
[demonitor-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L1159-L1185
[demonitor-receive]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6589-L6626
[demonitored]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3042-L3056
[dtor-walk]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L2940-L2947
[release-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L3352-L3369
[identity-remove]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14436-L14469
[exit-order]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14545-L14594
[exit-fire]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13806-L13833
[stop-wrapper]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3032-L3039
[env-doc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L573-L587
[env]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L629-L723
[stop-anomaly]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/common/erl_check_io.c#L1504-L1512
[stop-fast]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/common/erl_check_io.c#L1271-L1288
[dummy-dtor]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L2746-L2763
[takeover-calls]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L2634-L2664
[takeover]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L2811-L2875
[get-resource]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3225-L3261
[test-basic]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/nif_SUITE.erl#L1380-L1439
[test-self]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/nif_SUITE.erl#L1442-L1481
[test-demonitor]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/nif_SUITE.erl#L1536-L1564
[test-purge]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/nif_SUITE.erl#L1485-L1533
[test-down]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/nif_SUITE_data/nif_SUITE.c#L2991-L3026
[test-stress]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/nif_SUITE_data/nif_SUITE.c#L3376-L3444
[test-stress-down]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/nif_SUITE_data/nif_SUITE.c#L3461-L3512
[rust-owned]: https://doc.rust-lang.org/std/os/fd/struct.OwnedFd.html
[rust-borrowed]: https://doc.rust-lang.org/std/os/fd/struct.BorrowedFd.html
[rust-arc]: https://doc.rust-lang.org/std/sync/struct.Arc.html
[rust-send]: https://doc.rust-lang.org/std/marker/trait.Send.html
