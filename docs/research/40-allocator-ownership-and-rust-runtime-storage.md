# Allocator ownership and Rust runtime storage

Reviewed **2026-10-07** against Erlang/OTP **OTP-29.1.1**, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. The checkout was read only. **No allocator test, benchmark or Bream implementation was run or changed.** Rust contract references were checked against official documentation displaying **std 1.99.0**, revision `b940084d7`, on this date; that does not select Bream's minimum Rust version.

This extends [report14](14-beam-memory-native-io-source.md), [report18](18-process-execution-memory-deep-dive.md), [report34](34-message-allocation-and-copy-costs.md) and PERF-030 in the [performance backlog](../performance-plan.md). Those reports describe process/message ownership; this one follows what happens **after a runtime object requests raw storage and after its owners relinquish it**. It does not equate allocator reclamation with executing an object's destructor or completing process death. GC term traversal remains a separate investigation.

## 1. Principal call chains and the first ownership boundary

ERTS dispatches allocation by a generated memory-type identifier. Process records use FIXED_SIZE; heaps, old heaps, fragments and combined messages use EHEAP; message references and several timer records use FIXED_SIZE. The same memory type must be used on allocation and free. `erts_alloc` calls the selected allocator and treats failure as fatal, whereas `erts_alloc_fnf` returns its failure result. A portable runtime cannot infer recoverable per-process OOM from this wrapper. [Type map][types]; [wrapper contract and dispatch][wrapper].

Two concrete ingress points connect that machinery to processes: `erts_alloc_message` chooses a separately allocated message reference for size zero or an EHEAP combined allocation for a payload; `erts_free_message` reverses the corresponding representation. A process record reaches `erts_free(ERTS_ALC_T_PROC, p)` only after `erts_free_proc` asserts FREE state and zero process reference count. Those caller lifetime conditions precede allocator lifetime conditions; the allocator is not a replacement for them. [Message allocation][message-alloc]; [message release][message-free]; [reference allocator][message-ref]; [record release][process-free].

| Chain | Main branch followed | Boundary this review does not erase |
|---|---|---|
| Type → allocator interface | Setup selects system fallback, thread-specific, thread-preferred or locked interface. Only temp_alloc is eligible for the thread-specific interface; other configured per-thread families use preference. | Family/configuration determines the path; there is not one universal lock-free allocator. [Setup][setup]; [interface selection][interfaces]. |
| Preferred allocation | Current allocation index selects an instance; lock it if required; opportunistically drain delayed frees; allocate; on failure try additional draining and one retry. | A new allocation can pay for prior remote frees. “Local allocate” is not necessarily just a freelist pop. [Preference][pref]; [allocate][alloc-pref]. |
| Size/type dispatch | Fixed-size policy first; otherwise `size >= sbc_threshold` selects a singleblock carrier (SBC), smaller allocations a multiblock carrier (MBC). | This source equality branch is more precise than prose describing only larger/smaller sizes. [Size dispatch][size-dispatch]. |
| Preferred free | Resolve the block's current carrier owner. Free locally if it matches, otherwise enqueue deallocation there. | Current worker, current carrier owner and carrier's original owner are distinct identities. [Free][free-pref]; [owner lookup][owner]. |

Scheduler registration maps normal schedulers and dirty allocation instances separately. Normal schedulers handle their delayed-deallocation queues; dirty schedulers do not act as those handlers. Dedicated handler/auxiliary-thread registration covers the other instances. This matters to a host-embedded Rust runtime: progress of remote returns needs an explicit surviving consumer, not an assumption that the allocating process will run again. [Registration and handlers][handlers].

## 2. A remote free is a publication protocol

The remote path writes queue linkage, allocation type and flags into the surrendered block's user storage. Consequently, application payload use has already ended, but allocator producers may still hold pointers to that storage. A node is not reusable merely because the application considers it dead. [Queue representation][dd-block]; [enqueue metadata][enqueue-other].

The enqueue algorithm tries a release CAS on the observed tail's next pointer. A successful tail extender must publish the new tail and requests notification; a contending producer can splice into the intervening chain without requesting its own wakeup. Do not impose global FIFO semantics on concurrent frees or assume one notification per free. [Producer branches][dd-enqueue].

**Unmanaged producers have additional protection.** They read one of two reference-count indexes, increment its count, re-read the index, and retry if the epoch changed. They decrement after enqueue. The consumer advances its safe dequeue boundary only after both the recorded thread-progress value and the applicable unmanaged reference count permit it, with a memory barrier before advancing `unref_end`. A general Rust epoch scheme covering only worker threads would not automatically cover host threads making remote returns. [Unmanaged enrollment][unmanaged]; [consumer boundary][dd-boundary].

The consumer has two kinds of entries. Ordinary blocks are deallocated by type; a special header identifies a carrier “homecoming” event. Even ordinary blocks can need redirection: the consumer re-reads carrier ownership and queues the block again if ownership migrated while the request was in flight. Fixed-size blocks have a corresponding revalidation path. A routing address cached at the first free is not authoritative forever. [Consumer dispatch/homecoming][dd-consume]; [ordinary redirect][redirect]; [fixed redirect][fix-redirect].

The normal limited pass uses 20 iterations and opportunistic allocation work uses 2; the handler then visits applicable allocator families, and can additionally check pending carriers. Auxiliary work clears its flag before draining, reinstates work for backlog and arranges progress wakeups when entries are not yet eligible. These are iteration policies, **not a total allocator-call latency bound**, especially with locks, tree operations, carrier destruction and backing allocation inside them. [Limits][dd-limits]; [call sites][dd-entry]; [family loop][handlers]; [wake/continuation][aux].

## 3. Carrier reuse moves allocator ownership, not application values

An MBC contains multiple allocations. If a suitable free block is absent, creation may first fetch a carrier from a pool; otherwise it obtains backing from mseg/system allocation subject to configuration and fallback. A small allocation can exceptionally land in a forced-size system SBC when normal MBC creation fails. Thus a size-only prediction of physical representation misses an actual failure branch. [Free-block and emergency path][mbc-alloc]; [carrier creation][create].

A carrier can be pooled while it still contains live allocations. Abandonment checks allocator and carrier utilization, pool count, minimum free-block size, main-carrier exclusion, recent-activity suppression and outstanding progress protection. It removes the carrier from the instance's allocation structures, marks free portions discardable, inserts it into the pool and publishes ownership/flags. **Carrier migration changes management of existing memory; this inspected path does not relocate the live object bytes.** [Admission to pool][abandon-check]; [publication][abandon].

Fetching first examines an instance's own remembered pooled carriers, handling stale largest-free-size data, BUSY state and races. It then searches the shared pool and uses a CAS to claim an eligible carrier. Pool removal marks links deleted and records a future thread-progress value before the metadata can be freed or reinserted. The configured inspection counter is 100; concurrent traversal/retry behavior still requires separate analysis before converting that into a cost claim. [Own/shared fetch][fetch]; [unlink lifetime][pool-unlink]; [constants][pool-limits].

The owner lookup used by free has its own important race branch: it can lock its preferred allocator, try to mark a pooled carrier BUSY, then discover another allocator claimed the carrier and unlock again. A HOMECOMING transition uses a matching read barrier for the previous owner's write-barrier publication. These C-specific assumptions must not be copied as ordinary unsynchronized Rust field accesses. The source explicitly relies on a consistent block-header read despite adjacent flag updates; this review does not establish a Rust memory-model translation. [Owner revalidation/barriers][owner].

| State/transition | Permitted use | Deferred obligation |
|---|---|---|
| Live block in owned carrier | Application retains the allocation; instance manages surrounding free space. | No free/reuse of the block while application/runtime references remain. [Owner lookup][owner]. |
| Freed block queued remotely | Allocator queue uses the former user storage for linkage and type metadata. | Wait for queue eligibility; revalidate carrier owner before final free. [Queue boundary][dd-boundary]; [redirect][redirect]. |
| Partially occupied carrier pooled | Another instance may claim allocation management; existing object addresses remain. | Prevent simultaneous management through IN_POOL/BUSY/owner transitions. [Claim][fetch]; [owner][owner]. |
| Carrier removed from shared pool | New owner can use its blocks while carrier metadata remains alive. | Old pool readers may still require the removed links/carrier allocation. [Unlink][pool-unlink]. |
| Empty migrated carrier | Route home to its original instance, avoiding duplicate HOMECOMING requests. | Original instance handles pending pool bookkeeping and retirement. [Return home][home]; [consumer][dd-consume]. |
| Empty carrier awaiting progress | Origin can keep it on a pending-deallocation list. | Destruction waits for protection to expire; **its own allocator may instead reuse its free blocks without destroying stable carrier metadata**. [Pending destruction][retire]; [pending-list reuse][pending-reuse]. |

That last distinction is easily lost in a generic “epoch reclamation” summary. ERTS can reuse the interior of an empty pending carrier before destroying the carrier allocation; it cannot simply invalidate metadata that concurrent readers still observe. A Bream slab needs an equally explicit separation between slot occupancy, stale handles, page metadata and page lifetime. It does not need to adopt this particular carrier protocol.

## 4. Why a successful free need not reduce RSS

There are several different forms of retention, each with its own purpose:

- **Fixed-size caches.** Allocation can pop a per-type list. Free caches eligible local blocks; foreign blocks, busy pooled carriers, shrink mode and cache limits can instead take the underlying deallocation path. Cached `used`, `allocated` and list size are different counters. Periodic shrink is scheduled, while explicit flush uses a loop that bypasses the normal per-pass shrink limit. [Fixed alloc/free][fixed]; [shrink scheduling][shrink-schedule]; [shrink/flush][shrink].
- **MBC free space.** Free coalesces adjacent blocks. An entirely empty non-main carrier is destroyed; the main carrier remains on the reuse path. Normal builds restrict the examined discard operation to pooled-carrier frees; debug builds take a different branch. Discard preserves allocator metadata and selects aligned page ranges rather than invalidating the entire carrier. [Coalesce/main/discard choice][mbc-free]; [aligned discard][discard].
- **Retirement backlog.** Empty carriers can await thread progress, homecoming processing, or a bounded pending-carrier pass. Its 10-carrier limit is another local count policy. [Pending carriers][pending]; [return/retire][home].
- **Backing segment cache.** Even after `dealloc_carrier` calls its backing allocator, mseg may cache the segment and schedule a cache check rather than destroy it; a later allocation can reuse that segment. The system-allocator branch has its own policy outside this trace. [Backing dispatch][backing]; [mseg cache][mseg].

Therefore report application-live bytes, runtime references, queued remote frees, cached blocks, carriers/pages awaiting retirement, retained backing capacity and OS resident memory separately. Pool statistics themselves retry for consistency only finitely and can return the values obtained under mutation; they are not automatically an atomic snapshot. [Statistics reader][stats]. Retained reusable capacity is not inherently a leak, but an unconsumed remote queue or untracked shared allocation is not excused by calling it a cache.

## 5. Rust-specific opportunities and their proof obligations

These are **proposals**, not an arena decision or evidence that Rust is automatically faster than BEAM. A typed process subset can expose layout and ownership information at compile time, but raw allocation and destruction still execute native work.

**Typed slabs for runtime records.** Fixed-size process records, mailbox envelopes and continuation metadata are candidates for separate slabs with measured layouts. Explicit slot states can distinguish uninitialized, live, retired and reusable storage; a generation protects identity lookup only when its wrap/reuse policy is qualified. `MaybeUninit<T>` permits uninitialized storage but does not itself track initialized members or clean partially constructed values. Initialization state must survive any admitted suspension/failure boundary. [Rust MaybeUninit contract][rust-uninit]. This suggests eliminating unnecessary general allocator dispatch, not eliminating lifetime bookkeeping.

**Process-local regions for demonstrably local data.** A region containing only admitted plain values with no surviving external references might release storage by chunks after the process stops. A region containing buffer handles still owes their reference releases; separately allocated descendants still need ownership transfer or deallocation. General `Box<T>` cannot be fabricated from a pointer into an unrelated arena: its allocator/layout and valid-value requirements still apply. [Box layout][rust-box]. Generated typed handles or offsets could make the permitted ownership graph explicit, but this is a design experiment rather than approval of arbitrary arena-backed Rust values.

**Separate shared storage from producer-local regions.** The accepted immutable byte buffer can outlive its publisher. Its backing must remain owned independently of the publisher's region, or the region must remain alive for actual holders. Whole-region reference counting may retain unrelated dead objects, so compare retention as well as reference-count cost. Rust `Arc<T>` drops its inner value when the last strong owner disappears; choosing generic `T` does not restrict what that destruction does. [Arc contract][rust-arc]. Bream's opaque bytes contract must keep final release within qualified runtime code.

**Stable storage and cleanup are different obligations.** A pinned address-sensitive value must not be invalidated or repurposed before its required destruction. Leaking the storage forever can preserve that address promise, but it does not meet Bream's tracked reclamation requirement. Arbitrarily resetting an arena containing pinned values is not justified by Rust permitting leaks. [Pin drop guarantee][rust-pin]. `drop_in_place` requires a valid initialized value, proper alignment and exclusive destruction access; it executes the destructor, it does not budget or make it preemptible. [Drop contract][rust-drop]. Generated cleanup continuations should operate on admitted runtime representations instead of assuming every Rust `Drop` is safe or short.

**Allocator interception is not process admission.** A global allocator hook does not identify every owned object or its destructor, and ordinary optimized allocations need not invoke the hook at all. The allocator contract also prohibits unwinding and requires matching allocation/layout provenance. [GlobalAlloc contract][rust-global]. Explicit runtime allocation APIs can provide better accounting for admitted representations; replacing the host's allocator alone cannot make arbitrary crate code safely stoppable. Even a perfect freelist has native refill, page fault and backing allocator branches: this report supplies no hard bound for them.

## 6. Eight new branch cases

These cases extend the current EXEC-03.C01–C14 inventory and are **planned, unimplemented and unexecuted**. ERTS-specific branches are conditional reference targets, not required Rust data structures.

| ID | Trigger and required observation | Forbidden result / qualification |
|---|---|---|
| **EXEC-03.C15** | Cross SBC threshold at −1/exact/+1; force missing MBC free space, backing-allocation failure, emergency SBC and preferred-allocation retry after draining. Track real representation and failure policy. | Free through the wrong representation/type; assume every small block is MBC or every OOM is recoverable. [Dispatch][size-dispatch]; [fallback][mbc-alloc]; [retry][alloc-pref]. |
| **EXEC-03.C16** | Pause managed and unmanaged remote-free producers around epoch enrollment, enqueue and tail publication; let the consumer check eligibility and then resume producers. | Reuse queued storage while a producer can still touch its linkage; drop the unmanaged-reader obligation or strand the final small batch. [Enrollment][unmanaged]; [boundary][dd-boundary]; [aux][aux]. |
| **EXEC-03.C17** | Free remotely, migrate the carrier before consumption, then repeat migration/redirection for fixed and non-fixed records. | Free through stale owner, double account/free, lose the request, or cache a foreign fixed block through an ineligible branch. [Redirect][redirect]; [fixed redirect][fix-redirect]; [fixed policy][fixed]. |
| **EXEC-03.C18** | Pool fetch races BUSY marking/homecoming; include stale largest-free-size, removed candidate and two prospective owners. | Simultaneous carrier management, payload movement implied by ownership change, or stale metadata access. Trace failed claim/unlock and fallback. [Owner][owner]; [fetch][fetch]. |
| **EXEC-03.C19** | Free adjacent blocks in each order; leave one survivor, empty a non-main carrier, then empty main carrier; exercise discardable page boundaries. | Discard live bytes/allocator metadata, confuse coalesced free bytes with OS release, or destroy a retained main carrier through the ordinary free branch. [Coalesce][mbc-free]; [discard][discard]. |
| **EXEC-03.C20** | Fixed-cache churn then quiet period, shrink versus flush, foreign carrier return and segment-cache reuse. Trace application-live zero through all retention tiers. | Treat cached blocks as live application state or declare final OS release at logical free; leave required shutdown draining dependent on future application allocations. [Fixed/shrink][fixed]; [flush][shrink]; [mseg][mseg]. |
| **EXEC-03.C21** | Kill/fail during construction of a proposed typed slab/region object with initialized and uninitialized fields, local descendants and immutable handles already transferred to another process. | Drop uninitialized bytes, double-release moved ownership, free the recipient's backing or hide leak obligations under bulk reset. Rust-specific admission/cleanup probe, not an Erlang term oracle. [MaybeUninit][rust-uninit]; [Box][rust-box]; [Arc][rust-arc]. |
| **EXEC-03.C22** | Keep old slot/page-metadata observers while an empty carrier or proposed Rust slab is retired; compare permitted interior reuse with metadata destruction/reinsertion, plus pinned runtime records and stale handles. | Apply one reuse rule to distinct lifetimes, overwrite live/pinned records, or reclaim metadata before protection ends. A generation check alone is insufficient. [Unlink][pool-unlink]; [pending reuse][pending-reuse]; [Pin][rust-pin]. |

## 7. Five performance experiments

All are unmeasured. Preserve growing-mailbox and safe-stop semantics; none authorizes a capacity quota or dropping messages. Record compiler/build, allocator configuration, worker/host-thread counts, payload/layout mix, residence time, workload seed and observation mode. Include CPU-hog peers and shutdown with pending cleanup, not only a steady-state allocation loop.

| Local task | Baseline → treatment | Costs and cleanup/retention checks |
|---|---|---|
| **PERF-40.T01: typed record storage** | Separate general allocations → type-specific slabs for qualified runtime records. | Alloc/free cost, cache footprint, metadata bytes, idle cost per worker, refill/failure tails and fragmentation. Qualify C15/C21/C22; keep stale-handle and partial-initialization checks. Accept only if retention after churn and full tracked cleanup remain explainable. |
| **PERF-40.T02: local-state regions** | Individually owned local objects → chunked regions for a compiler-proved local subset, with explicit external-resource ledger. | Construction, lookup, kill-to-stop and stop-to-cleanup; count chunks and actual handle releases. Vary few large versus many small objects. C21 must pass; compare region retention when any shared backing/recipient survives. Reject an “O(1) cleanup” claim that excludes its ledger or backing frees. |
| **PERF-40.T03: remote-return policy** | Locked shared return queue → owner-routed/batched return queues with explicit host-thread participation and final-batch wakeup. | Atomic retries, lock time, notifications, redirections, queued bytes and retirement delay under skew/migration. C16/C17/C18; model ownership before removing locks. A faster producer cannot hide a slower or starved reclamation consumer. |
| **PERF-40.T04: cache and page policy** | Simple nonmigrating slab/page cache → measured thresholds for cache shrink, empty-page return and optional page ownership transfer. | Refill rate, fragmentation, resident versus reserved bytes, discard/refault cost and idle-worker retention. C18/C19/C20/C22; preserve live addresses and retained metadata. Compare against simply disabling page migration; BEAM's policy complexity is not an initial requirement. |
| **PERF-40.T05: explicit cleanup scheduling** | Opportunistic frees/refills on application turns → separately accounted bounded runtime cleanup steps and pre-refill work. | Longest admitted segment, peer-ready delay, cleanup backlog, kill-to-stop, final release and total CPU through drain. C15/C16/C20/C21; include failure and final-owner release. Raw allocator calls remain measured indivisible operations unless a separate resumable implementation is qualified; moving them to a worker does not prove a bound. |

## Evidence limits and next trace points

Read upstream scenarios, **not executed**: `coalesce.c` checks previous/next/both-neighbor merges; `cpool.c` interleaves threaded pool insert/delete and checks final emptiness; `migration.c` tracks carrier employers through growing/shrinking/cleanup workloads. The suite runs migration under multiple allocation strategies and can skip its allocator tests under some memory-checker builds. These bodies motivate fixtures; their existence neither proves every race nor reports a new passing run. [Coalescing test][test-coalesce]; [pool test][test-pool]; [migration test][test-migration]; [suite configuration][test-suite].

Traced: representative process/message allocation entry points, interface selection, preferred allocation/free, remote queue enrollment/drain/redirection, principal carrier fetch/abandon/homecoming/destruction, fixed-cache paths and the backing mseg cache decision. Sampled: pool linked-list lifetime, allocation strategies through their function-pointer interface, statistics and upstream tests. **Unreviewed:** complete free-block strategy implementations, every realloc branch, allocator-stop and all online/offline-worker protocols, weak-memory proof across architectures, full mseg/mmap/system-allocator behavior, native resource callbacks and Rust provenance proof for a concrete storage implementation. No arena, slab layout, pool size, latency bound or production allocator is selected here.

Local navigation: [preferred free](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_alloc_util.c:6216), [remote queue eligibility](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_alloc_util.c:1970), [carrier ownership lookup](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_alloc_util.c:1735), [pending-carrier reuse](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_alloc_util.c:3737).

[types]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc.types#L115-L142
[wrapper]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc.h#L199-L301
[message-alloc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.h#L573-L595
[message-free]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.h#L629-L635
[message-ref]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.c#L40-L43
[process-free]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12141-L12149
[setup]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc.c#L697-L726
[interfaces]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc.c#L885-L938
[pref]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L1711-L1723
[alloc-pref]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L6105-L6137
[size-dispatch]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L5985-L6026
[free-pref]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L6216-L6238
[owner]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L1728-L1821
[handlers]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc.c#L1895-L1992
[dd-block]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.h#L385-L398
[enqueue-other]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L2224-L2236
[dd-enqueue]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L1842-L1891
[unmanaged]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L1915-L1938
[dd-boundary]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L1941-L2003
[dd-consume]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L2073-L2221
[redirect]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L2322-L2376
[fix-redirect]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L2023-L2065
[dd-limits]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L68-L76
[dd-entry]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L2302-L2320
[aux]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L2078-L2179
[mbc-alloc]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L2380-L2421
[create]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L4125-L4286
[abandon-check]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L2254-L2299
[abandon]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L3967-L4000
[fetch]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L3558-L3735
[pool-unlink]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L3493-L3511
[pool-limits]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L3183-L3191
[home]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L3811-L3863
[retire]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L3866-L3902
[pending-reuse]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L3737-L3755
[fixed]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L1388-L1473
[shrink-schedule]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L1342-L1357
[shrink]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L1476-L1528
[mbc-free]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L2687-L2807
[discard]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L2613-L2656
[pending]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L3764-L3807
[backing]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L4488-L4527
[mseg]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/common/erl_mseg.c#L690-L737
[stats]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_alloc_util.c#L4030-L4076
[test-coalesce]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/alloc_SUITE_data/coalesce.c#L100-L127
[test-pool]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/alloc_SUITE_data/cpool.c#L82-L159
[test-migration]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/alloc_SUITE_data/migration.c#L184-L285
[test-suite]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/alloc_SUITE.erl#L49-L94
[rust-uninit]: https://doc.rust-lang.org/std/mem/union.MaybeUninit.html#initializing-an-array-element-by-element
[rust-box]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
[rust-arc]: https://doc.rust-lang.org/std/sync/struct.Arc.html
[rust-pin]: https://doc.rust-lang.org/std/pin/index.html#subtle-details-and-the-drop-guarantee
[rust-drop]: https://doc.rust-lang.org/std/ptr/fn.drop_in_place.html#safety
[rust-global]: https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html#safety

The proposed cases are integrated in the [build plan](../build-plan.md); the report-local experiments feed the stable [performance task backlog](../performance-plan.md). The [Rust opportunity map](../rust-opportunities.md) compares these mechanisms without selecting a production representation.
