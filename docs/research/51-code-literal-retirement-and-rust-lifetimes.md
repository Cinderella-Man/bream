# Code, literal retirement and Rust lifetimes

Source review, 2026-10-08. Reference: OTP `29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`, verified against the local `upstream/otp` checkout. This closes selected literal-lifetime branches left open by [report 23](23-process-exceptions-code-lifetime.md), using the reclamation vocabulary from [report 46](46-thread-progress-and-retirement-domains.md). Evidence is **source-read and upstream-test-read only**. No OTP tests, Bream tests or performance experiments were executed for this report.

The practical boundary is that retiring executable code, removing a process's literal references, and freeing the original literal storage are separate events. Static native Rust can omit code replacement. It still needs correct lifetime and release accounting for owned messages, shared immutable buffers and retained diagnostics. Dynamic code replacement remains conditional later research; the cases below do not promote it into the first implementation.

## 1. Closing entry into old code precedes inspection

The purger prepares, checks processes, then completes or aborts. Preparation records each affected fun's old address before installing unloaded dispatch; abort restores dispatch before clearing the pending address. [Purger phases][S01]; [fun-entry transitions][S02].

Code staging excludes conflicting loads while pending fun markers exist. Thread progress separates publication from inspection and, on abort, restoration from resumption. Completion releases executable storage and queues its literal area separately. [Preparation/abort][S03]; [completion][S04].

The resumed pending-fun trampoline applies the fun again: an aborted purge can succeed; successful purge may produce an exception or invoke the loader when code is absent. Merely retaining a closure value does not guarantee that the original executable body remains callable. [Pending-fun trampoline][S05].

**Rust implication:** if native code were ever unloaded, its lifetime would include return addresses, suspended frames, function pointers and generated cleanup/vtable code. A lifetime-safe payload handle alone would not establish that all future executable entry paths have closed. This is a design obligation, not a claim that Rust has an automatic hot-unload facility. For the static implementation, prevent the unsupported replacement operation at the API boundary instead of introducing unused purge states.

## 2. Message evacuation has an ordered boundary

Ordinary message copying snapshots the currently retiring literal area. The copier preserves references to other literals, but copies objects in that area. A new sender cannot continue creating the same unprotected area references after the publication protocol has taken effect. [Send setup/copy][S06]; [copy-size discrimination][S07]; [copy discrimination][S08].

`handle_cla` processes the queued messages preceding its copy request. Its source explains that later messages cannot contain references into the retiring area because the request passed through the signal queue after publication. Each affected message gets an attached fragment holding its evacuated data; putting the copied object only on some unrelated process heap would violate message self-containment. Both direct message terms and existing fragment chains are examined. [Ordered boundary and fragment copy][S09].

The scan can yield using message-queue markers. Once it reaches its boundary, it either checks heap-GC need within the remaining allowance, requests a later check, or delays its continuation. The marker state and reply must survive these distinct branches without rescanning an unbounded prefix or declaring completion early. [Scan continuation][S10]. The larger scheduling result is a source-based inference: an implementation that scans an arbitrary current queue snapshot without coordinating future sends has not reproduced this safety argument.

**Rust implication:** an owned envelope or runtime-owned immutable handle can make the sender/receiver lifetime explicit and avoid evacuation altogether. A borrowed pointer into a replaceable data arena needs equivalent retention or relocation. `Send` alone says nothing about the lifetime of an arena fabricated behind an unsafe pointer or the runtime's ability to release a killed process's handles.

## 3. Heap roots and backing allocations require different work

The retirement check covers stack, young/old/abandoned heaps, dictionary, heap fragments, matched-message fragments and direct argument-register references. GC-disabled construction postpones inspection; stale non-live registers are range-checked rather than dereferenced. [Root check][S11].

Large checks become dirty system work. GC-disabled tasks remain pending; literal collection can itself move to dirty execution. Dirty maintenance orders literal work before other GC work to avoid redundant collections and only then services the waiting tasks. [Normal system-task dispatch][S12]; [dirty ordering][S13].

Literal GC first performs an ordinary major collection. It then reserves old-heap space based on the literal area's size, copies the area to temporary writable storage, relocates roots and collects reachable literals from that temporary copy. The scratch copy is necessary for this collector's destructive forwarding markers. It is not evidence that every Rust representation must copy an entire immutable data region. [GC preparation and scratch area][S14]; [root and heap relocation][S15].

Reached off-heap binaries, magic references and external-node entries acquire ownership for the receiving process. Later, releasing the original area drops its own off-heap ownership before freeing the area. Thus the small literal wrapper and its potentially larger backing allocation have different lifetimes. [Ownership acquisition][S16]; [literal-area release][S17]. The source accounts GC work after evacuation and restores hibernation when applicable. [Collection completion][S18].

Evacuation can increase a process's charged heap enough to trigger the configured maximum-heap action; this path can mark it exiting with `killed`. [Heap limit][S19]. A different Rust ownership representation may avoid that temporary allocation, but must record its resource-accounting mapping explicitly. “Cheaper representation” is not permission to hide retained storage or silently promise identical byte-for-byte BEAM heap accounting.

## 4. A copy reply can precede final literal access

Before exit cleanup replies to system tasks, a process blocks literal release if its exit reason is non-immediate or dirty execution might still use heap terms. The obligation covers future retiring areas too. The blocker is released after off-heap, heap and dictionary cleanup. [Exit enrollment][S20]; [final cleanup][S21].

The collector correlates requests by area and limits outstanding system requests. Only two GC-copy jobs are allowed concurrently; queued GC work remains included in the outstanding total. It advances after completing all requests for the area, not merely after dispatching them. [Collector orchestration][S22].

Area switching publishes the next area and rotates scheduler blocker counters. After thread progress, the old counter set can only drain; actual freeing waits for those blockers. A new area can start while an old blocker remains. Unblocking wakes via scheduled auxiliary work. [Switch/wait/free][S23]; [block release/wakeup][S24].

**Rust implication:** the existing distinction between death notification and reclamation completion is necessary here. A reclamation domain must include helpers, exit payloads and escaped owners that still touch retired storage. An acknowledgment may end one protocol phase while another runtime-owned token keeps the storage alive. `Arc` can model shared ownership, but the runtime still needs to account for each owner independently of arbitrary application destructor execution; safe code may deliberately forget a value. [Arc ownership][R02]; [destructor limits][R03].

## 5. What the upstream tests establish when read

| Upstream fixture read | Relevant intended observation | Evidence limit |
|---|---|---|
| `code_SUITE:t_copy_literals` and `t_copy_literals_frags` | Repeated module reload/purge overlaps literal message traffic, fragment-bearing messages and queued receiver work. [T01] | Source fixture only; no stress result from this checkout. The fragment receiver's displayed receive pattern has five list elements while the sender constructs six; do not treat that branch as demonstrated per-message value validation. [T02] |
| `provoke_mem_leak` / `create_binaries` | Hold repeated references to literal binaries and bitstrings across purge, validate values, then explicitly GC holders before relying on their termination for this fixture's deallocation check. [T03] | The explicit GC is part of the fixture. It does not turn arbitrary `DOWN` delivery into a general backing-storage reclamation barrier. |
| `code_SUITE:max_heap_size` | A process retains a literal list, deletes/purges its module, and the test expects `killed` under a small configured heap limit. [T04] | Test-read evidence for that configured branch, not a measured universal kill timing bound. |

The five-versus-six element test observation is confined to this pinned source. No upstream bug claim or test execution follows from it; a future reference run should preserve the fixture and separately validate the intended receiver branch.

## 6. Nine additional conformance cases

**CODE-01.C05–C13 are planned, unimplemented and unexecuted.** They refine report 23's coarse cases with new retirement transitions. Code-purge triggers apply only if dynamic replacement is admitted; owned/shared data safety applies to the initial static runtime too. When translating a case, record whether it tests executable retirement, replaceable data retirement, or ordinary final-owner release.

| Case | Trigger and required/forbidden observation |
|---|---|
| **CODE-01.C05** | A fun invocation races purge preparation and then soft-purge abort. Publish the pending-entry state before inspection; restore dispatch and complete its visibility boundary before clearing suspension state. No new old-code entry after the inspection boundary and no caller stranded by abort. [S02], [S03], [S05] |
| **CODE-01.C06** | Successful purge returns while another process or queued message retains literal data. Separate executable removal, queued evacuation and storage reclamation; data remains readable until its own ownership protocol completes. A Rust static configuration rejects replacement without invalidating existing constant/message values. [S04], [S09], [R01] |
| **CODE-01.C07** | Send literal-bearing messages immediately before/after the copy barrier while the receiver scan yields. Preserve each message's contents and order, attach/retain its own surviving data, and retain the scan markers. Never free the area based on an unordered snapshot or let post-barrier sends resurrect its unowned references. [S06]–[S10] |
| **CODE-01.C08** | References survive only in a matched-message fragment, abandoned heap, dictionary or direct argument register; repeat during GC-disabled construction and dirty work. Include all admitted holders, defer unsafe inspection, and distinguish a pending check from completion. A conservative unnecessary collection is safer than following stale register pointers. [S11]–[S13] |
| **CODE-01.C09** | A small live literal belongs to a large area; evacuation crosses a configured heap limit, or the holder was hibernated. Account temporary and retained storage, preserve the declared failure policy and restore the applicable suspended state. Do not report copy volume, heap charge and final live bytes as one number. [S14], [S18], [S19], [T04] |
| **CODE-01.C10** | More holders require GC than the allowed concurrent jobs; initial replies, queued jobs and unrelated/stale-area replies interleave. Maintain area/request correlation and total unfinished work; no advance while a holder remains unprocessed. Correctness must survive a different qualified Rust work limit. [S22] |
| **CODE-01.C11** | A terminating process replies while a non-immediate reason or dirty helper can still access literals, including an area queued later. Acquire the blocker before the reply and release it only after the last covered access. Death notification or task acknowledgment alone cannot authorize reclamation. [S20], [S21] |
| **CODE-01.C12** | Back-to-back areas rotate counters while a terminating process stalls; race its last unblock with collector waiting and a newly queued area. Preserve wakeup, counter-generation identity and the visibility boundary; allow useful work on the next area without prematurely freeing the previous one. [S23], [S24] |
| **CODE-01.C13** | Several holders retain a binary/sub-binary after its original area retires; some holders die during transfer and one survives. Establish each backing owner before releasing the old one, release each obligation once and keep the surviving view valid. Final wrapper removal does not imply final backing allocation release. [S16], [S17], [T03] |

## 7. Four Rust performance experiments

These are hypotheses, not measured improvements. Use Rust baselines with the same observable data-lifetime contract so removing unsupported hot-code machinery is not presented as a speedup over a baseline that never needed it. Record allocation count, copied bytes, atomic operations, retained capacity, peer-ready delay, kill-to-stop and stop-to-cleanup separately. Dynamic-data treatments do not require or authorize executable unloading.

| Experiment | Baseline → treatment and workload | Compiler/API/runtime obligations and rejection checks |
|---|---|---|
| **CL51-T01 — Statically known immutable constants** | A correct general runtime buffer handle for all constant payloads → generated typed `Static(&'static [u8])`/owned/shared variants, with the static path avoiding heap allocation and reference-count traffic. Sweep tiny/large fixed literals through send, fanout and kill; measure code-size and branch costs too. | Rust static items have program lifetime; only genuinely immutable permanent storage qualifies. Private construction must prevent leaking dynamic allocations and calling them free static storage. Runtime-owned mutable/shared cases retain normal accounting. Reject hidden permanent allocation growth, interior mutability, or a win that comes only from omitting the dynamic case. [R01] |
| **CL51-T02 — Explicit holder accounting for replaceable data** | A correct arena-retirement implementation enumerates admitted process/message roots → compiler-generated typed handle fields plus runtime-maintained owner records identify the relevant holders, avoiding global unrelated-state scans. Sweep many processes with sparse holders, repeated transfers and diagnostic retention. | Rust borrows can prevent safe references escaping a valid owner; generated fields do not discover raw/native escapes. API admission must close those escapes, and runtime publication must register ownership before visibility. Cleanup records survive forced termination. Reject missed matched-message/diagnostic/helper owners, double-release, holder-index contention that worsens peer tails, or strong-count snapshots used as a reclamation proof. [S09], [S11], [S20], [R02], [R03] |
| **CL51-T03 — Constant-region ownership granularity** | Each live view retains one entire immutable data region → typed per-payload owners, or a measured copy-out threshold for small surviving values. Workload: huge immutable region with one tiny long-lived slice, many shared large slices, and partial fanout followed by sender death. | Owner-plus-offset views must remain bounds checked and retain their allocation; moving the wrapper does not extend a bare slice lifetime. Compiler field ownership can make the relationship explicit, while actual last-owner tracking and cleanup remain runtime duties. Compare atomic cost/allocation/cache overhead against retained capacity. Reject a lower reported process heap that merely moves memory to an unreported global owner. [S16], [S17], [R02] |
| **CL51-T04 — Non-destructive typed evacuation** | A qualified Rust tagged-region copier uses whole-region scratch plus root relocation → generated layout visitors or index-based immutable graphs copy only reachable data with an external forwarding table. Conditional workload: replaceable data arenas with sparse and dense liveness, diamonds/shared children, deep graphs and interruption at each step. | Rust-owned immutable source plus private destination construction can prohibit destructive source writes; the API must preserve sharing/value semantics and exclude unsupported pointers. Runtime retains both source and unfinished destination across yields and kill, accounts bytes/edges, and publishes only complete values. Reject unbounded traversal, lost shared ownership, higher dense-case tails, or avoiding scratch by dereferencing already-retired storage. [S14]–[S16] |

Static typed paths can remove dynamic representation work. `Arc<T>` supplies shared ownership, not immutability for every possible `T`, automatic process admission, or an unconditional destructor/cleanup guarantee. Any native callbacks or cleanup code reachable from a purported data-only region remain a separate admission question. [R02], [R03]; [Rust admission report](41-rust-guarantees-and-process-admission.md).

## Coverage and remaining gaps

**Traced:** selected prepare/abort/complete transitions, send-side literal discrimination, message and heap evacuation, delayed/dirty maintenance, backing ownership, collector work correlation, exit blockers and final-area switching. **Test-read:** literal stress, binary survival and max-heap fixtures. **Not executed:** reference observations or Rust benchmarks. **Unclosed:** complete loader/on-load/NIF/JIT teardown, every literal-bearing signal producer, persistent-term retirement callers, all GC shapes, full atomic proof and the fragment fixture's apparent receiver-pattern mismatch. These are source boundaries, not claims of complete CODE-01 coverage. No new user decision or ADR is needed to preserve the existing static-code scope.

## Primary source anchors

All OTP links below are immutable anchors into the verified commit. Rust primary documentation was checked on 2026-10-08; the standard-library pages identify Rust 1.99.0.

[S01]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erts_code_purger.erl#L120-L151
[S02]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_fun.c#L159-L240
[S03]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_bif_load.c#L2100-L2219
[S04]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_bif_load.c#L2248-L2326
[S05]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erts_code_purger.erl#L74-L103
[S06]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.c#L719-L797
[S07]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/copy.c#L83-L120
[S08]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/copy.c#L703-L723
[S09]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L8146-L8289
[S10]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L8293-L8338
[S11]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_bif_load.c#L1006-L1114
[S12]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10878-L10925
[S13]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L11128-L11179
[S14]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1160-L1246
[S15]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1248-L1308
[S16]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1323-L1373
[S17]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_load.c#L683-L716
[S18]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1375-L1389
[S19]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_gc.c#L1197-L1210
[S20]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14370-L14402
[S21]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13445-L13481
[S22]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erts_literal_area_collector.erl#L27-L159
[S23]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_bif_load.c#L1810-L1946
[S24]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_bif_load.c#L1693-L1723
[T01]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/code_SUITE.erl#L1187-L1268
[T02]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/code_SUITE.erl#L1270-L1299
[T03]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/code_SUITE.erl#L973-L1030
[T04]: https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/code_SUITE.erl#L1455-L1486
[R01]: https://doc.rust-lang.org/reference/items/static-items.html#items.static.lifetime
[R02]: https://doc.rust-lang.org/std/sync/struct.Arc.html
[R03]: https://doc.rust-lang.org/std/mem/fn.forget.html#safety
