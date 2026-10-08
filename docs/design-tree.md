# Design tree and interview

Research started 2026-10-07 from `/home/bat/projects/rust-actor-runtime-lab`, commit `f78658b08faaea84b7c9e7bb3a2dab13fafc17c7` (clean at inspection). The user authorized deep investigation, naming, and creation of a new project under `/home/bat/projects/`. This is a research project. The primitives-first scope is accepted; the production execution engine and public API remain open.

**Interview rule:** before every behavior question, research BEAM first. Clear BEAM behavior is to be replicated and documented without asking the user to choose it again. Only a researched ambiguity or concrete conflict with Rust safety or an accepted requirement warrants a behavior question. The standing instruction is recorded in [CONTEXT.md](../CONTEXT.md#working-rule-research-beam-before-asking) and takes precedence over the question backlog below.

## Settled in this conversation

- **Purpose:** investigate extracting a reusable Rust library that makes sense independently of the Elixir migration platform, analogous to Tokio's independent usefulness.
- **First workload:** user selected “Long-lived stateful services (Recommended)”.
- **Required behavior:** user clarified “like elixir - it should somehow swap between CPU hogging processes”. This means resumable scheduling preemption. Terminating a computation when it consumes fuel is not a substitute.
- **Native Rust library:** user clarified “This is pure Rust code, pure Rust library - BEAM-like green thread behaviour library but it has nothing to do with running some foreign code or Elixir code”. The investigation now targets native Rust computations, not a Wasm guest product.
- **No cooperative requirement:** user emphasized “the point of this library is that it's PREEMPTIVE SCHEDULING - requiring yielding is completely missing the point - I can just use Tokio then”. Application-written yields/awaits are excluded as the mechanism that delivers the core promise.
- **Initial native compatibility:** user accepted runtime-aware I/O and synchronization while preserving automatic preemption of ordinary CPU loops.
- **Initial platform:** user accepted Linux x86-64 first.
- **Build experience:** user accepted an ordinary Cargo dependency and standard compiler first. Compiler instrumentation is a possible renegotiated fallback, not the default product.
- **Safe forced termination:** user selected “Safe forced termination is also essential”. This is an additional first-version requirement, not satisfied by the pause/resume probe or by ordinary stop requests.
- **Ownership:** user accepted process-owned state and owned messages rather than shared mutable state across processes. Enforcement remains technical work.
- **Kill cleanup:** user accepted BEAM-style runtime-managed cleanup: stop the process and reclaim runtime-owned resources without promising arbitrary user destructor execution. This is a desired contract, not an already-proven native implementation.
- **Initial library scope:** BEAM/ERTS process primitives in Rust, including processes, mailboxes, messaging, monitoring, linking and lifecycle control. GenServer, supervisors and restart policies are excluded from the first scope by the user’s explicit correction.
- **Message model:** typed Rust protocols with payload-independent lifecycle identity.
- **Exit semantics:** BEAM-style linked-exit propagation and trapping, with direct forced kill remaining untrappable.
- **Receive semantics:** selective receive consumes the earliest matching message and retains unmatched messages, with an optional timeout; long scans participate in automatic scheduling.
- **Mailbox policy:** BEAM-style growing mailboxes and ordinary local sends that do not wait for receiver consumption or capacity; bounded admission is a separate later policy.
- **Reference workload:** room/session processes using those primitives directly.
- **Deliverable:** findings in a newly named local project. Bream is the user-selected working name as of 2026-10-07, not a published or reserved package.

## Questions asked

1. First use case: answered, long-lived stateful services.
2. Trusted versus hostile code: not answered as a security choice. The user's reply corrected the focus to CPU switching. Do not infer hostile-code isolation, hard resource quotas, or a trust model from that answer.
3. Native Rust compatibility versus separately compiled process code: answered by the native-library and mandatory-preemption corrections above. Wasm remains evidence from the source lab and a technical comparison, not the recommended product boundary.
4. Runtime-aware channels, locks, and I/O: answered “Runtime-aware I/O and sync are acceptable (Recommended)”. This does not prove allocator/TLS safety or authorize a manual-yield requirement.
5. First platform: answered “Linux x86-64 first (Recommended)”. Other platforms are later ports rather than first-release requirements.
6. Build experience: answered “Ordinary Cargo dependency first (Recommended)”.
7. Forced termination versus scheduling only: answered “Safe forced termination is also essential”. This changes the product feasibility gate: safe stop/reclamation must be designed and qualified, not deferred out of scope.
8. Process-owned message passing versus cross-process shared mutable state: answered “Process-owned state and messages (Recommended)”.
9. Forced-termination cleanup: answered “BEAM-style runtime-managed cleanup (Recommended)”. The runtime must account for and safely reclaim its resources; indefinite quarantine is not recorded as an accepted substitute.
10. Generated process model: answered “Yes, investigate a generated process model (Recommended)”. A Cargo attribute that generates mandatory checkpoints and restricts process-safe operations/types is an accepted research direction. Its exact supported language and production implementation are not selected.

11. Familiar developer experience: answered in favor of Rust APIs, with an explicit correction that the first scope is **BEAM primitives**, not OTP behaviours. Earlier recommendations for a GenServer/supervision milestone are superseded by [ADR 0004](adr/0004-beam-primitives-first.md).
12. Reference service: answered “room session”. The initial fixture uses raw room/session processes rather than a GenServer or supervisor framework.

13. Mailbox message model: answered “Typed Rust protocols (Recommended)”. Lifecycle identity and relationships remain independent of the payload type.
14. Mailbox admission: answered “BEAM-style growing mailbox (Recommended)”. The first experiments use ordinary non-waiting local send and measure queue growth; this is not an unlimited-memory guarantee. [ADR 0005](adr/0005-typed-growing-mailboxes.md).

15. Receive semantics: answered “Selective receive + timeout (Recommended)”. Retain unmatched messages and schedule long scans automatically.

16. Exit semantics: answered “BEAM-style exits and trapping (Recommended)”. Normal/abnormal exit rules, trapped notifications and direct untrappable kill follow the reference semantics. [ADR 0006](adr/0006-beam-exits-and-trapping.md).

## Dependency tree

```text
Standalone library + resumable CPU fairness [settled requirement]
├── Native Rust execution [settled requirement]
│   ├── signal-driven library → Linux x86-64 first [accepted]; FFI/TLS/allocator rules open
│   ├── ordinary Cargo build [accepted] → compiler route is fallback requiring a changed decision
│   └── runtime-aware I/O/synchronization [accepted] → safety and embedding audit
├── Meaning of tiny [measured costs available; product target open]
│   ├── idle and suspended process cost
│   └── realistic state, mailbox and I/O cost
├── Scheduling behavior [CPU hogs must resume]
│   ├── progress under worker saturation
│   ├── fairness target and tail latency
│   └── block/receive/wake/stop state transitions
├── Generality [primitives-first boundary accepted]
│   ├── processes, mailboxes, send/receive, monitoring, linking and lifecycle control
│   ├── raw room/session reference fixture [selected]
│   └── OTP behaviours/supervisors [outside initial scope]; distribution/migration [later]
└── Operational guarantees [dependent frontier]
    ├── accidental misuse versus adversarial code [security promise not selected]
    ├── safe forced termination [required] → owned state/messages + runtime cleanup [accepted]
    └── identity, observability and overload
```

The technical investigation can proceed across alternatives. Accepted scope decisions do not select an execution engine; further architecture ADRs require an explicit choice. Research recommendations must remain distinguishable from user decisions. The earlier lab's message-boundary upgrade decision concerns its actor platform; it does not silently impose hot upgrade support on this smaller library.

## Current frontier after the kill requirement

The user accepted investigating Cargo-based generated process code with a restricted/checked set of callable native operations and data types. The next design decision is whether a sufficiently useful and enforceable supported surface can be built. No production macro API or custom language has been accepted. A macro that inserts checks only in visible loops is insufficient to cover arbitrary dependency calls, operators, callbacks and destructors.

Before another implementation choice, establish whether that restricted boundary can deliver all accepted requirements for useful programs. Signal preemption has passed a mechanism probe; the generated subset now also demonstrates stopping an infinite process and reclaiming its tracked buffer. General safe program construction, owned-message transfer and full resource lifetimes remain unqualified. Hard scheduling/kill deadlines, native density targets, public API stability, hostile-code security and a concrete production engine remain open. These are documented unknowns, not silently chosen guarantees.

## Current planning session: BEAM primitives in Rust

The user selected Rust APIs and the room/session reference workload, then explicitly corrected the previous roadmap’s scope: BEAM primitives and OTP behaviours are different layers. Bream starts with process execution, mailboxes, messaging, monitoring, linking and termination. GenServer, supervisors, restart strategies and application frameworks are outside the initial scope. [ADR 0004](adr/0004-beam-primitives-first.md) records that decision.

Primary-source reviews 07–09 remain useful comparisons, but their recommendations to build a supervised service early are superseded. The boundary review in report 10 and revised [roadmap](roadmap.md) govern the current plan. No new experiment has been implemented.

Q13–Q16 are all answered: typed protocols, growing mailboxes, selective receive with timeout, and BEAM-style exits/trapping. [ADR 0005](adr/0005-typed-growing-mailboxes.md) and [ADR 0006](adr/0006-beam-exits-and-trapping.md) record these semantic choices.

Q17 asked for final shared understanding, but the user instead requested more grilling and a local source-level BEAM review. That finalization question is superseded, not accepted or still awaiting an answer. The interview is reopened around the facts found in the pinned Erlang/OTP source. Exact public syntax, representation of system notifications alongside typed messages, supported data/helper operations and performance targets remain open; research recommendations are not user decisions.

## Source-review round

The user explicitly authorized cloning Erlang/OTP and tracing the implementation of the runtime elements. The checkout is `upstream/otp`, release `OTP-29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. Research covers scheduling and generated checkpoints, mailboxes and signal queues, lifecycle relationships and exit cleanup, process memory, native operations, timers and I/O. This is source review, not a claim that every ERTS subsystem has been audited or that the source has been executed.

Questions downstream of those implementation facts will be asked as the relevant traces are established. Earlier accepted requirements remain in force, including native Rust, automatic CPU switching, safe forced termination, typed protocols and BEAM primitives before OTP behaviours.

The first source-informed round is Q18–Q20, asked together because their decisions are independent:

- **Q18 answered:** “One runtime envelope (Recommended)”. Typed application messages plus `Down`/trapped `Exit` share one selective mailbox order; internal control remains separate from user receive. [ADR 0007](adr/0007-unified-runtime-envelope.md).
- **Q19 answered:** “Allow immutable buffers (Recommended)”. Runtime-managed immutable bytes may be shared through owned references, with no shared mutable application state or arbitrary shared types. [ADR 0008](adr/0008-immutable-buffer-sharing.md).
- **Q20 answered:** “Death, then tracked cleanup (Recommended)”. `Down` means execution stopped and the process is no longer live/addressable, with tracked reclamation completion separately observable. This excludes BEAM's dirty-NIF post-death execution exception. [ADR 0009](adr/0009-death-and-reclamation-observation.md).

Q18–Q20 are settled. The next source-informed round was asked as Q21–Q23:

- **Q21 answered:** “do whatever beam does - in general this should work as close to BEAM / Elixir”. Ordinary local send to a dead process discards the message. More broadly, derive observable primitive defaults from BEAM instead of asking the user to redesign them; ask only about genuine Rust-specific tradeoffs or necessary departures. [ADR 0010](adr/0010-beam-observable-semantics-by-default.md).
- **Q22 pending:** restrict the first guaranteed process model to approved/generated helpers, or also design external host-native jobs that can outlive the caller. Recommendation: qualify helpers first; do not silently weaken process preemption/kill for dependency compatibility.
- **Q23 pending:** select useful CPU work for the first checked model. Recommendation: byte-payload parsing/transformation plus room membership scanning/fanout; numerical or user-named work is an alternative.

BEAM constants, lock structures and algorithms remain evidence, not silently adopted Bream implementation policy. Proposed later questions in individual research reports are a research backlog, not additional questions already asked. Q21 resolves their ordinary-behavior alternatives in favor of BEAM: for example, demonitor does not implicitly flush an already-materialized DOWN; explicit flush does. Rust-specific identity/representation safety and transitive code admission still need engineering decisions and evidence.

## Behavior-completeness audit

The user asked whether exact behavior had been established in every case and edge case. **No:** source coverage, the scoped reference specification, the Rust mapping and executable conformance evidence are different things, and none supports a blanket completeness claim. The [coverage audit](behavior-coverage.md) records those distinctions and the remaining obligations.

The follow-up audit resolves reference facts without asking the user: mailbox/timeout/ordering cases, local link/monitor/exit cases and scheduling/resource-failure boundaries. It already corrected the earlier claim that absent-target linking always produces an asynchronous `noproc` exit; a local non-trapping caller can receive an immediate catchable error instead. It also corrected the description of nominal signal-work budgets as exact maxima. The new case inventories distinguish one required result from a set of allowed concurrent outcomes.

No new behavioral preference question is warranted merely because these cases were previously missing from our notes. Earlier unanswered Q22–Q23 do not block this audit or source-derived semantics; before revisiting any question, apply the standing BEAM-first rule. Remaining implementation questions must be tied to a concrete unresolved Rust constraint and the reference behavior it must preserve.

## Comprehensive build-plan request

The user next requested all these branches as a comprehensive build plan and feature list, including edge cases. The initial [build plan](build-plan.md) recorded 23 feature groups and 199 stable branch cases across admission/conformance, execution/resources, messaging and lifecycle. It adds implementation dependencies, incremental exit gates, forbidden observations, evidence status and a separate later backlog. All features remain planned, and ambiguous reference cases or unresolved Rust mappings remain explicit qualification gates.

This request authorizes documenting the implementation path; it does not select a production engine, public API, arbitrary-Rust admission, hard latency target or host-job escape. No further behavior preference question is needed to document clear BEAM defaults. The next implementation frontier is executable reference fixtures and a useful, enforceable process boundary.

## Deeper process-runtime scope request

The user requested deeper coverage of everything directly related to processes and explicitly excluded bundled utilities such as ZIP and cryptography. This clarifies the research boundary: execution, owned state/resources, addressing, messaging, relationships, death, timing and observation belong here; application-library algorithms do not. A Rust dependency called from a guaranteed process still needs qualified scheduling and cleanup integration. BEAM/ERTS primitives remain distinct from OTP behaviours.

The [expanded inventory](process-runtime-inventory.md) and reports 18–25 trace these areas with source/test limits. All 21 declared control-signal kinds are mapped at both live/exiting dispatch. The build catalogue grows from 23 groups/199 cases to **40 groups/337 cases**, preserving prior IDs and explicitly marking extensions and representation research. No new runtime tests were run, no production engine or public API was selected, and no shared mutable state, quotas, native escape, distribution or hot upgrade was silently accepted.

Source facts continue to settle behavior without preference questions. Unexecuted boundary predictions and incomplete concurrency/ownership arguments remain work items. The next practical frontier is exact-version oracles for affected local semantics and useful public Rust admission; optional features have their own later experiment batches.

## Protocol-depth and executable-reference request

The user asked to investigate further. Reports26–28 deepen the suspension consumer, process lookup/reclamation and priority receive-marker protocols. Report29 builds the pinned reference core in a separate clone and executes bounded fixtures on release JIT with one/four workers and the debug interpreter with four workers. Several source predictions are observed; the optimized priority-marker fixture reproduces a debug assertion while its unoptimized control succeeds. No upstream patch/issue was submitted and the original clone remains unchanged.

The [plan](build-plan.md) now has 40 groups and 366 stable cases. The 29 additional cases refine existing groups. Facts were established without asking behavioral preference questions; no engine, API, shared-state escape, distribution feature or OTP behaviour was selected. Source/documentation disagreements and crashes remain explicit compatibility research, not implicit desired behavior. Bream implementation and safety qualification still remain open.

## Ownership and wakeup follow-up

The user requested further depth. Reports 30–32 follow buffered ingress across receiver death, receive parking/timer wakeup, and relationship cancellation into their ownership and ordering protocols. Report 33 executes two finite sequentially consistent abstractions, with counterexamples for five deliberately broken variants. Source, model and earlier VM observations remain separate evidence; no new upstream VM run, source patch or Bream implementation occurred in this pass.

At that stage the catalogue reached 40 groups and 392 stable cases, preserving all previous IDs. The 26 additions refine initial obligations and explicitly conditional optimizations; they do not add OTP behaviours or choose a production architecture. The context glossary now distinguishes cancellation barriers and pending unlink. Clear reference facts required no preference questions. The late-buffer-reinstallation path remains an unresolved source concern with a concrete controlled experiment, not a diagnosed defect or user decision.

## Performance-directed source investigation

The user requested that every further investigation consider speed improvements and add notes and tasks. This is now a persistent [context working rule](../CONTEXT.md). Reports 34–37 deepen message allocation, immutable bytes, scheduler load balancing, timer service and native checkpoint/state-save costs. The build plan now has 40 groups and 423 cases; the separate [performance backlog](performance-plan.md) has 36 planned experiments with baseline/treatment, dependencies and rejection conditions. No speedup was measured, production engine selected or scope extension accepted.

Independent review removed a duplicate pending-monitor case and corrected frame-pointer-mode state restoration and the retained-priority-prefix fixture wording. Steal and timer helper batch limits must not be mistaken for end-to-end scheduler bounds. The late-buffer-reinstallation concern remains an unconfirmed source hypothesis. Clear reference behavior required no new user questions.

## Deeper review with Rust-specific opportunities

The user requested continued breadth and explicit attention to Rust advantages. Reports 38–41 trace collector roots/relocation, free-ID and spawn publication, allocator ownership/retention, and compiler-guaranteed versus runtime-enforced properties. Twelve isolated language fixtures compiled in two profiles; 24 compilations and 12 executions produced expected diagnostics/outputs. These are evidence about the language, including accepted safe-Rust counterexamples, not a Bream process implementation.

The [opportunity map](rust-opportunities.md) and new tasks PERF-037–054 compare typed state, consuming transfers, private construction states, guarded lookup, slabs/regions and cleanup obligations. That pass brought the plan to 447 branch cases and 54 performance tasks, retaining earlier IDs. New glossary terms distinguish process admission, address publication and cleanup obligation. Review preserved exact initialization accounting under coarser reclamation and separated queued/visible infant records from application execution.

The decision frontier remains technical: demonstrate a useful enforced admission boundary, qualify borrowed/pinned state, and compare explicit resource ownership alternatives. No new API, engine, region allocator or generation-width policy was chosen; no ADR was created. No behavior-preference question is needed for clear reference facts. Unconfirmed rare identity branches remain research tasks, with no claimed upstream reproduction.

## Deeper I/O, observation and callback investigation

The user requested further breadth with Rust-specific improvement opportunities. Reports 42–44 follow Linux polling, consistent enumeration and trace-session teardown, and resource monitor/callback/destructor ownership. Report45 executes five sequential Linux ABI scenarios in two compiler profiles: 2 compilations and 10 executions with expected observations. Source inspection, Linux probes and earlier BEAM/Rust/model evidence remain separate.

The plan now has 474 cases and 70 performance tasks. New terms distinguish readiness notification, registration lifetime and observation ownership. The source review corrects blanket failed-registration rollback and distinguishes last public session handle from internal retained metadata. Rust can check ownership transfers and specialize approved records/handlers; runtime claim, stop and reclamation protocols remain necessary. No engine, polling representation, callback extension surface or optimization was selected, and no bundled library or OTP behaviour entered scope.

There is no new preference frontier for these facts. The next technical work is useful admission first, then owner-death/cancel/close evidence when I/O is added. Broad observer/session and native-code replacement cases remain conditional. No new BEAM defect or speedup was reproduced; exceptional source branches remain qualified research gaps.

Independent review preserved the ownership distinction in PERF-064: automatic session destruction follows loss of all public and temporary owning holds; tracee/index attachment must not itself keep a session active. The syscall fixtures and all 31 recorded input/artifact/raw-stream hashes were reviewed without re-execution; no material probe issue was found.

## Deeper concurrency, locks and clock domains

The user requested continued research and Rust-specific opportunities. Reports 46–48 trace thread progress, process locks/handoff and time domains. Report49 executes seven isolated Loom variants across three projections: three complete and four weakened controls fail as intended. The catalogue reaches 500 planned cases and 85 experiments, preserving previous IDs and historical evidence.

New glossary terms separate reclamation domain, quiescent interval, lock authority and clock domain. No preference question is needed for clear source behavior. Unclosed facts are explicit experiments: full weak-memory/reclamation composition, participant failure/shutdown, complete lock graph/backend qualification and clock-provider fault cases. No new engine, lock layout, epoch library, clock mode or OTP behaviour was selected. Source-level anomalies remain unconfirmed; no new upstream failure was reproduced.

Independent review found no material issue in the Loom source/results and verified all 27 recorded hash pairs. This validates the recorded projections, not a production memory model or speedup.

Independent review of PERF-071–085 and the opportunity-map additions found no material semantic drift, unsupported speed claims or initial-scope expansion. Source-only and model evidence remain distinct.
