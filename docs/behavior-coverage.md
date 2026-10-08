# Behavior coverage: what is established and what is not

**Status: incomplete, 2026-10-07.** We have not established exact behavior in every case, even within the initial local-process scope. The source walkthrough is substantial, but it is neither a complete executable specification nor a qualified Bream implementation. The pinned reference remains Erlang/OTP `29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`; [source provenance](../evidence/beam-source-review/source-pin.json), [coverage of inspected subsystems](beam-source-map.md).

The user's standing rule applies: look up BEAM first and inherit clear observable behavior. An unanswered technical question is work for this investigation; it is not a reason to ask the user to reinvent a process primitive.

The [build plan](build-plan.md) now assigns this work to 40 feature groups and 537 stable branch cases, including conditional extension/reference work. It includes implementation tasks, dependencies, allowed/forbidden observations and qualification gates. The catalogue documents known branches and open obligations; it does not change the incomplete execution and Rust-mapping status below.

## Three different completeness claims

| Claim | Current state |
|---|---|
| We know the reference contract for every supported operation and edge case | **No.** Many common and subtle cases have source-backed answers; the operation/error/race inventory is still incomplete. |
| We have a precise Rust mapping for every reference observation | **No.** The mailbox envelope and several ownership choices are accepted, but public construction, error classes/reasons, resource operations and shutdown still lack a complete specification. |
| Bream has passed tests establishing the required behavior | **No.** The two earlier mechanism probes cover narrowly admitted execution. The process/mailbox/link/monitor runtime does not yet exist. Targeted exact-version BEAM fixtures now ran, but the complete upstream suite and cross-runtime conformance suite did not. [Results](research/29-pinned-reference-observations.md). |

The generated probe explicitly describes itself as internal research machinery, not a public process-safe API. Its context and futures are ordinary Rust types; its successful cases and compile-rejection fixtures do not establish a safe general admission boundary. [Probe source](/home/bat/projects/bream/experiments/generated-process/runtime/src/lib.rs:1), [probe scope](../experiments/generated-process/README.md).

## An exact contract may permit several outcomes

BEAM guarantees signal order from the same sending entity to the same destination under the stated delivery conditions; it does not supply a universal order across independent senders. Runtime services may themselves consist of independently executing entities. Therefore conformance sometimes means admitting a set of legal observations and forbidding others, not predicting a single global trace. [Pinned signal-order contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L495-L512), [runtime-service ordering](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L259-L278).

Examples include independent senders racing, a separate monitor installation racing target death, and message arrival racing receive timeout. The specification must state the ordering assumptions and allowed outcomes. Wall-clock timestamps alone are not a universal tie-breaking rule. An implementation detail observed at one pinned commit is not automatically a language guarantee.

## Edge-case inventories

| Family | Source audit | Remaining qualification |
|---|---|---|
| Ordinary send, ordering, self-send, selective clauses/guards, timeout and waiting | [Mailbox cases](research/15-mailbox-edge-cases.md), [implementation trace](research/12-beam-mailboxes-source.md) | Complete receive/arrival/timeout/kill interleavings, typed guard admission, payload ownership and error mapping; no Bream executable results. |
| Spawn, self/repeated/dead-target monitors and links, demonitor/flush, unlink/relink, exits | [Lifecycle cases](research/16-lifecycle-edge-cases.md), [implementation trace](research/13-beam-lifecycle-source.md) | Exhaustive transition/error mapping and resource ownership through races, including catchable errors versus runtime death; no Bream executable results. |
| Scheduling, long built-ins, memory pressure, native work and final reclamation | [Execution/resource cases](research/17-execution-resource-edge-cases.md), [scheduler trace](research/11-beam-scheduling-source.md), [memory/native trace](research/14-beam-memory-native-io-source.md) | Transitive Rust admission, longest indivisible operations, panic/abort/OOM boundaries, reclamation progress and realistic costs. |

These inventories enumerate concrete cases; their row counts are not a completeness percentage or a proof that no other edge case exists.

## Corrections found by checking the earlier notes

**Linking an absent local process is not always asynchronous failure.** If a local PID is already absent at the early check and the caller is not trapping exits, `link/1` raises a catchable error-class `noproc` exception. A trapping caller or an absence discovered later can instead take the exit-signal path. The earlier lifecycle report oversimplified this and has been corrected. The Rust API must preserve this distinction rather than mapping every failure to a later `Exit` notification. [Reference contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L3866-L3879), [source branch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L155-L183).

**A signal-work allowance is not an exact maximum cost.** Runtime operations are charged and limits checked at operation boundaries; weighted operations and accounting can overshoot a nominal allowance. The earlier mailbox report's maximum wording has been corrected. Neither this accounting nor the 4,000-reduction constant establishes a hard wall-clock latency bound. [Operation charging](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6338-L6347), [post-operation limit check](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6968-L6971), [scheduler analysis](research/11-beam-scheduling-source.md).

These corrections are evidence that the existing notes need continued adversarial review, not a basis for claiming that all remaining cases are now settled.

**One source/documentation discrepancy now has exact-version fixture evidence.** The documented `demonitor(..., [flush])` equivalence removes one matching tuple. For a still-active ordinary one-shot process monitor, however, the pinned C path can remove the origin and return without running the flush helper. The targeted fixtures in [report29](research/29-pinned-reference-observations.md) confirm an application-created matching tuple remains in that case on all three tested VM configurations, while the documented expanded expression removes it. The typed Rust mapping and other variants remain unresolved. The [lifecycle audit](research/16-lifecycle-edge-cases.md) records the fixture and the unresolved mapping to Bream's typed envelope. [Documented equivalence](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L2076-L2089), [conditional flush path](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L639-L654).

## Deeper process audit

The [expanded inventory](process-runtime-inventory.md) adds eight reports and 138 cases. It covers priority and proxy scheduling, counted suspension, heap/fragment ownership, both hibernation forms, aliases/priority/markers, message timers, spawn requests and options, names, late controls, inspection, diagnostics, I/O lifetime, exceptions/code references and process-owned external state. Bundled compression/crypto algorithms remain excluded.

One finite coverage claim is now checked: all **21 declared internal control-signal opcode kinds** appear in both live and exiting dispatch maps. The [matrix](research/24-process-signal-dispatch-matrix.md) records their actions; the [extracted inventory](../evidence/process-deep-review/signal-dispatch-inventory.json) verifies the declared/handled sets. This is not complete coverage of each helper, subtype, race or architecture.

The follow-up [protocol/fixture review](research/29-pinned-reference-observations.md) adds 29 branch cases and executes selected earlier predictions on the pinned source build. Paused timer lookup identity, hibernation arity, synthetic flush and a controlled mixed notification setup now have observations; a priority-marker fixture reproduces a debug VM assertion. Reports26–28 deepen suspension, lookup/reclamation and receive-marker invariants. Full interleaving coverage, Rust mapping and native safety remain open. Later APIs do not expand the first release implicitly.

## Highest-priority open Bream obligations

1. **Admission and transitive execution.** Every accepted helper, operator, guard, allocation, copy and collection operation must have a qualified suspension/termination contract. Generated loop checks do not cover a nonreturning opaque call. Ordinary safe public construction must not bypass this boundary.
2. **Error and exit representation.** Map normal completion, explicit process exit, catchable reference errors, runtime exit signals and Rust panics without conflating them. Specify how application-created lifecycle-shaped messages and reference-scoped flush matching relate to the typed envelope; do not silently assume either forgeable or private notification constructors. Rust `panic=abort`, allocator failure and host defects do not become isolated process failures merely because a PID exists.
3. **Ownership during transitions.** Specify who owns each payload, queue node, partial helper result and immutable-buffer reference during send/selection/death. Address aliasing, final release, stale wakeups and exactly-once reclamation.
4. **Death and cleanup observations.** The user selected stopped execution and dead identity before `Down`, with tracked cleanup separately observable. Enumerate which runtime resources are invalidated before notification and prove cleanup does not access freed state, run arbitrary user destruction indefinitely or starve peers.
5. **Concurrent lifecycle operations.** Turn monitor/death/demonitor/flush, unlink/relink/exit, and spawn/initial-relationship races into explicit state models. A stress test alone cannot establish every possible interleaving.
6. **Identity and host lifetime.** Specify runtime-instance identity, slot reuse/generation exhaustion, handles surviving runtime shutdown, spawning during shutdown, and shutdown invoked while processes are executing. BEAM is itself a runtime; embedding it as a Rust library introduces host-lifetime edges that do not have a one-line BEAM API equivalent.
7. **Capacity and failure paths.** Distinguish process-table exhaustion, per-process tracked limits, mailbox growth and host allocation failure. Growing mailboxes are an accepted semantic choice, not a promise of infinite memory or recoverable global OOM.
8. **Scheduling evidence.** Establish useful state and input-sized work on one worker, then cross-worker ownership and wakeup correctness. Record work accounting, peer delay, kill-to-stop and cleanup latency separately. Measured maxima do not establish hard deadlines.

The reference facts should continue to be researched directly. Rust implementation gaps should be resolved through design and qualification; user input is needed only for an actual remaining ambiguity or conflict with accepted requirements.

## Scope boundaries are also part of the contract

The first target remains ordinary local process primitives with typed application protocols, normal scheduling, growing mailboxes, selective receive/timeouts, links, monitors and lifecycle control. Priority messages, distribution, aliases, broad timer/name APIs, ETS, ports and code upgrades remain later or explicitly scoped investigations. They are not OTP behaviours, but absence of their full edge-case coverage does not silently expand the first milestone. GenServer and supervisors remain above the requested runtime layer.

“As close to BEAM/Elixir as possible” supplies the behavioral default for supported primitives. It does not mean an unimplemented capability is already supported, that Rust types are Erlang terms, or that native dirty-work exceptions override the stronger accepted Bream stop guarantee.

## Required shape of a conformance case

The [latest protocol pass](process-runtime-inventory.md) adds 26 cases covering buffered publication/retirement, parking/timer wakeup and relationship cancellation. [Two finite models](research/33-finite-ownership-interleavings.md) have been executed with five broken controls. This adds bounded model evidence; it does not execute the complete ERTS paths or settle Bream's Rust mapping. The new buffer-reinstallation concern is a source hypothesis requiring controlled reachability/reclamation evidence, not a confirmed bug.

Before claiming a primitive is specified, each important case needs:

- A supported operation, initial state and ownership assumptions.
- Inputs and ordered events; identify events intentionally left concurrent.
- One expected result or an explicit set of allowed results, plus forbidden observations.
- A pinned reference contract/source/test and a label distinguishing guarantee from implementation detail.
- The Rust-facing mapping, including failure and cleanup observations.
- Evidence status: reference test inspected, reference test executed, Bream test executed, bounded interleavings explored, or still unresolved. These are separate fields, not one “verified” checkbox.

The next research deliverable is the [numbered build-plan inventory](build-plan.md) made executable for the scoped primitives. Reference fixtures should establish allowed observable outcomes; Bream tests should check the same contracts plus Rust ownership obligations. No blanket “every edge case is figured out” claim is justified by the current evidence.

## Performance-source coverage update

Reports 34–37 add 31 cases for message allocation/freeze/retention, scheduler scan/retry/plan lifetime, timer placement/cancellation/expiry loops, and checkpoint/save-state fast/slow paths. The [performance plan](performance-plan.md) separately records 36 planned experiments with evidence gates. Selected source branches and test bodies were inspected; no new VM run or benchmark occurred.

Two cautions are now explicit: the successful-steal cap is not a visited-node/retry bound, and a high-level timer batch can rearm a wheel callback before the outer scheduler regains control. The timer path remains a source prediction needing branch-confirmed measurement. Neither observation supplies a measured production regression. The existing buffer-reinstallation concern remains unconfirmed. [Current documentation validation](../evidence/process-performance-review/review-validation.json).

## Rust ownership/source coverage update

Reports 38–41 add 24 planned cases: six collector-root/relocation refinements, eight identity/spawn-publication refinements, eight allocator/storage refinements and two Rust admission/borrow cases. At that stage the catalogue reached 447 cases, with 54 separately numbered performance experiments. Source inspection and the [isolated language probes](research/41-rust-guarantees-and-process-admission.md) remain different evidence classes: expected Rust diagnostics and outputs do not establish the Bream API, schedulability or resource closure.

Rare full-ID-reuse link ownership and maximum-table sentinel/free-pool interactions are explicitly unresolved in report39. No new upstream runtime failure was reproduced. [Validation for that pass](../evidence/rust-ownership-review/review-validation.json) checks documentation and retained probe artifacts, not exhaustive behavior or safety.

## Linux I/O and observer/resource lifetime update

Reports 42–44 add 27 refinements to IO-01 and OBS-01/02, bringing the catalogue to 474 cases. They cover failed interest changes, stale readiness, error routing, backend accounting, resource callback claims/destruction, snapshot deletion history and observer teardown. IO-01.C06 now requires the actual post-failure state rather than presuming transactional rollback for every backend operation. All cases remain planned; broad diagnostic and native-extension APIs remain conditional scope.

[Report45](research/45-linux-io-observations-and-rust-boundary.md) records two compilations and ten executions of five sequential Linux ABI scenarios. These are neither Bream tests nor BEAM reference fixtures. They establish controlled observations about event copies, descriptor reuse/duplication, nonblocking retry, one-shot rearm and hangup with unread data. Concurrent dispatch-to-close races, injected epoll errors, native callback teardown and observer cleanup tails still need execution. [Validation for that pass](../evidence/io-lifetime-review/review-validation.json).

## Thread progress, process locks and clocks

Reports 46–49 add 26 planned cases: nine coordinator/retirement refinements, eight process-lock ownership branches, eight clock/deadline branches and one weak-memory harness qualification. At that stage the catalogue reached 500 cases and 85 planned performance experiments. The source work does not require copying BEAM's five locks, global participant registration or every clock mode into the initial runtime.

[Report49](research/49-rust-atomic-protocol-observations.md) records one release build with locked dependencies and seven isolated Loom invocations. Three positive variants complete, while four weakened controls fail at their expected invariant; no watchdog or exploration limit explains those failures. All modeled payloads are atomic and raw storage retirement is absent. No new BEAM fixture, clock fault injection, native lock-backend run or Bream implementation occurred. [Current validation](../evidence/concurrency-clock-review/review-validation.json) checks documentation and retained artifacts, not full safety or coverage.

## System-task, literal-lifetime and naming coverage update

Reports 50–52 add 23 planned cases: eight system-task/native-return branches, nine code/literal-retirement branches and six registry/typed-routing branches. At that stage the catalogue reached 523 cases and 99 planned performance experiments. The original OTP checkout and 375 historical evidence files remain unchanged.

[Report53](research/53-name-routing-reference-and-variance-observations.md) records eight local VM families in each of three profiles, including 300 total registration race rounds. Four Rust compilation attempts produce two running covariance counterexamples and two expected invariant-conversion rejections. These do not establish all race schedules, typed-registry correctness or general process admission. Task completion and literal-retirement branches remain source/test-read only. The apparent upstream literal fixture pattern mismatch is recorded as a source limitation, not a newly executed defect. [Current validation](../evidence/branch-continuation-review/review-validation.json).

## Worker service, async lifetime and measured Rust alternatives

Reports 54–55 add 14 planned cases, reaching 537; the performance backlog has 107 tasks. No new BEAM execution occurred in this pass. Source/test-read findings distinguish physical wake from dispatch permission, offline service ownership, task abort from native completion, keyed invocation order from callback routing, and VM exit from embedded shutdown.

Reports 56–57 add executed evidence: 567 ownership-loop rows; two completing and two deliberately deadlocking Loom variants; 432 measured queue invocations plus 36 warmups over three retained stages. The final queue study corrects startup/order confounds and records intended-arrival delay, but offers no backend winner. Loom here does not inject spurious wakes or model full repeated enrollment. Static-payload timing includes seven local improvements, three regressions and 26 inconclusive comparisons; Bream admission/kill/tail latency remains unqualified. [Current validation](../evidence/worker-service-review/review-validation.json).
