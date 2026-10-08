# Name resolution, registry snapshots and typed routing

Research date: 2026-10-08. Reference: OTP `29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. This extends [report 20's registry lifetime trace](20-process-lifecycle-deep-dive.md) with address-form dispatch, enumeration and Rust protocol checks. [Report 53](53-name-routing-reference-and-variance-observations.md) records separately executed local VM and compiler observations. Registration remains an extension; this does not add naming or distributed delivery to the initial process milestone.

**Resolving a name once and sending through that name later are different operations.** Removing repeated lookup can be valid for an explicitly captured process address. Silently caching the first destination of a name changes rebinding behavior. Rust's typed protocols introduce an additional obligation: a replacement binding cannot inherit permission to receive a different Rust payload merely because it has the same spelling.

## Address form determines both resolution time and failure behavior

The atom-destination branch of `do_send` resolves the current registry entry, looks up a process or port, and returns `SEND_BADARG` if neither lookup succeeds. The two-element `{Name, Node}` branch with the local node also resolves the registry, but its failed lookup returns the ordinary discard result. Thus the missing atom form and the missing local tuple form are not interchangeable. This is a source-backed compatibility detail, now observed with both release JIT and debug interpreter VMs. It does not establish remote-node behavior. [Atom dispatch][S01] [Local tuple dispatch][S02]

| Operation | What is selected | What later rebinding can change |
|---|---|---|
| `whereis(Name)` | The identifier copied at lookup time, or `undefined` | No retargeting of that returned identifier. Its process can independently die. |
| `Name ! Message` | The binding consulted for this send | A subsequent send can choose another incarnation; a missing atom destination errors. |
| `{Name, node()} ! Message` | The binding consulted for this local tuple send | A subsequent send can choose another incarnation; an absent destination is discarded in this branch. |
| `monitor(process, Name)` | The incarnation selected when establishing the monitor | Unregister/rebind does not redirect the monitor; absence produces `noproc` for that request. |
| Name-targeted message timer | The binding consulted at expiry | Rebinding before expiry can change the receiver; capturing the original PID changes this contract. |

These cut points follow the ID-only lookup, monitor producer and timer callback. The lookup helper holds registry read protection while copying the ID, but does not reserve the name or supply continuing liveness. It does not test `EXITING`; the separate process-returning helper has optional lock/reference retention and an exiting-state check. A safe Rust return type must make clear whether it returns an identity value or an actual protected record view. [Lookup variants][S03] [Named monitor][S04] [Timer resolution][S05]

The implementation resolves a name and then performs a process-table lookup; do not infer an atomic binding-plus-delivery transaction or a guaranteed race winner. The executed fixtures use ordered setup for their required observations. Existing NAME-01.C03–08 retain the broader death/rebinding and lock-lifetime obligations.

## Enumeration has a different ownership and cost boundary

`registered_0` obtains registry read protection, reacquires the caller's main lock if the safe-lock path released it, computes space from the registry count, allocates the result and traverses the table before releasing registry protection. Empty enumeration returns immediately. The result contains copied atom names, not borrowed table-entry pointers. The traced membership is collected with writers excluded; it is not a promise that these names remain registered when the caller uses the list. Neither the documented API nor this traversal supplies a sorted-name contract. [Enumeration][S06] [Public result][S07]

Allocation and full traversal occur inside that read-protected interval. This identifies a potential writer-delay cost; it is not a measured regression or proof that every allocation invokes GC. A Rust `Vec<NameId>` might reduce result construction cost, but a `Vec<&Entry>` escaping the guard would instead create a lifetime obligation. Snapshotting sharded tables one shard at a time also needs an explicit consistency argument; it cannot automatically claim the same single protected membership interval.

The existing registry already hashes and compares atom identities: its hash is the atom index, and equality compares the term value. Interning names in Rust should therefore be compared with an integer-key baseline as well as a string-key baseline. It cannot be described as eliminating repeated string hashing from this BEAM path. Allocation, token lifetime and name cardinality still cost something. [Atom-index hash][S08] [Equality and table setup][S10]

## Typed routing is an admission boundary, not a cast

Two viable research alternatives remain open. A registry can enforce one protocol for a name's entire permitted binding lifetime, or it can check protocol identity when resolving a particular binding. The former needs an explicit name/protocol policy across unregister/rebind. The latter must return a checked typed destination tied to the incarnation actually checked; retaining a name plus an old type check and resolving again during send is insufficient. No public mismatch error or namespace policy is selected here.

The same issue crosses timer expiry: a timer that resolves by name at delivery cannot trust only a protocol check made when the timer was created if incompatible rebinding is possible. Lifetime monitors, on the other hand, must stay payload-independent. Protocol mismatch must never cause unchecked message reinterpretation, accidental delivery to the old owner, or a second effective binding hidden in a `(name, type)` namespace presented as BEAM's single namespace.

`TypeId` is an opaque in-process type identity, not a durable wire/schema identity. Its layout, hashes and ordering are not a cross-release contract. Its official documentation also warns that covariance can bypass constructor-established type invariants. The new Rust control reproduces that bypass with function-pointer subtyping; an invariant control rejects the conversion. Neither control implements process admission, name rebinding or concurrent lookup. [Rust TypeId contract](https://doc.rust-lang.org/std/any/struct.TypeId.html) [Executed controls](53-name-routing-reference-and-variance-observations.md)

Runtime name tokens also need runtime-instance and lifetime validity. Reusing a compact slot after unregister is safe only if retained name values and pending timers cannot resolve the slot as a different spelling. This is separate from PID-generation safety. A guard protects an entry for a bounded interval; a copied owned name token must carry whatever lifetime or generation contract its representation requires. These are proposed Rust obligations, not evidence that BEAM's permanent atom identity has this reuse problem.

## Additional planned cases

All cases remain **planned** Bream qualification/reference work. The VM fixtures provide bounded observations for selected reference paths, not complete case coverage.

| Case | Branch and required treatment | Forbidden result and acceptance evidence |
|---|---|---|
| NAME-01.C09 | Send to absent atom name versus `{Name, local_node}` and dead PID; repeat after unregister. Preserve the supported address-form failure/discard distinctions. | Forbid one generic missing-target rule for every addressing form. Use report53's ordered local controls; remote distribution remains separate. |
| NAME-01.C10 | Resolve a typed name, then rebind to another incarnation/protocol before use; include deferred named timer delivery. Validate the selected binding or enforce a declared stable protocol policy. | Forbid unchecked payload reinterpretation, stale type authorization and implicit per-type duplicate namespaces. Mismatch API remains unresolved; require constructor/variance and rebind controls before enabling the extension. |
| NAME-01.C11 | Enumerate empty/nonempty registry, unregister after obtaining result, and race enumeration with churn. Return owned valid names with the qualified snapshot consistency. | Forbid dangling entry borrows, assumed sorted order or continuing target liveness. Verify copied-result lifetime and writer progress; compare concurrent histories against the declared snapshot interval. |
| NAME-01.C12 | Compete to register two names for one target, and one name for two targets, particularly across different proposed shards. | Forbid a successful pair violating either uniqueness direction or inconsistent target backpointer. Single-key map insertion alone is insufficient. Reference races are observed; a sharded Rust transaction still needs modeling. |
| NAME-01.C13 | Retain compact name tokens in snapshots/timers while unregistering and recycling registry storage, including another runtime with identical numeric tokens. | Forbid old-name-to-new-spelling aliasing and cross-runtime resolution. Qualify interning/retirement or nonreused identities; no fixed token width or allocator chosen. |
| NAME-01.C14 | Optimize repeated name routing with cached typed destinations while replacement and concurrent send occur. Distinguish an explicitly captured PID from a name resolved per operation. | Forbid changing dynamic-name semantics to sticky first-owner delivery. Validate binding version/type and record lifetime as one protocol; forced invalidation races need model/negative controls beyond sequential report53. |

## Rust performance experiments

These are proposals, without measured gains. All require the registration extension's correctness gates and the shared [performance acceptance protocol](../performance-plan.md).

| Candidate | Baseline → treatment | Evidence and rejection gate |
|---|---|---|
| Name keys | Owned string lookup and an interned integer baseline → compact owned name tokens with qualified lifetime | Repeated routing plus high-cardinality churn: hash/allocation cost, token retention and cleanup tails. Reject wrong-runtime/reused-name resolution; report interning cost separately. |
| Typed resolution | Checked erased binding on each lookup → one validated concrete typed destination for explicit resolve-once use | Measure dispatch, type-check cost and code size; compare identical resolution semantics. Dynamic names still revalidate their binding. Reject covariance bypass, incompatible rebinding or sticky name caches. |
| Registry sharding | One correct registry lock → name shards plus coordinated per-process registration ownership | Same-name and same-process contention, churn and death: throughput, retries, writer/kill tails and retained records. Reject either uniqueness violation, lock-order cycle or stale backpointer. |
| Owned enumeration | Correct protected owned-name copy → compact token vector with separately qualified allocation/version strategy | Sweep registry size and writer load: lock-held time, copied bytes, allocation and peer latency. Reject escaping borrowed entries or silently weakened snapshot consistency. |
| Routing cache | Correct per-operation typed resolution → version-validated cache with safe record retention | Hot stable names mixed with rebinding and expiry bursts: cache hits, invalidation traffic, retained records and tail latency. Reject the check/use race and indefinite retention; compare against explicitly cached PID only as a different workload. |

## Coverage limits

Read registry/send/monitor/timer source and the upstream registration stress test. That upstream test is not executed here; report53 runs smaller independently authored controls. Full port lock restarts, remote names, atom allocation, all snapshot schedules, typed registry implementation and performance remain unqualified. [Upstream stress test][S09]

[S01]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L2506-L2536
[S02]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L2613-L2655
[S03]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/register.c#L277-L408
[S04]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L774-L808
[S05]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_hl_timer.c#L1145-L1199
[S06]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/register.c#L562-L604
[S07]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L5912-L5927
[S08]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/register.c#L37-L49
[S09]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/register_SUITE.erl#L48-L66

[S10]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/register.c#L115-L160
