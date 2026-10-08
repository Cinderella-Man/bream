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

## Resumed research: system work, literal storage and name routing

On 2026-10-08 the user asked to recover the checkpoint and continue original BEAM implementation branches, retaining the Rust performance focus. The recovered baseline was reports 1–49, 500 cases and 85 tasks. Existing BEAM-first authorization governs this continuation; moving over branches means extending the source/behavior/Rust-mapping research, not silently selecting a production engine.

Reports 50–53 add 23 cases and 14 performance tasks, reaching 523 and 99 respectively. Three source reports distinguish internal work from request settlement, literal reclamation from code removal, and dynamic names from captured process identity. Reference naming fixtures and Rust variance controls were executed separately. The glossary now names binding and resolution explicitly. No new ADR or settled architecture choice follows.

Typed registry protocol policy/mismatch results remain unresolved extension design. Broad native jobs and dynamic code stay conditional, and the accepted stop-before-Down rule still excludes the dirty-NIF post-death application exception. No behavioral preference question blocks clear source research. The next factual frontier is in the [inventory](process-runtime-inventory.md#latest-checkpoint-and-next-source-frontier).

## Responsiveness-first experiment priority — Q24

On 2026-10-08, after source review established that BEAM does not supply Bream with a native-library latency target, the user answered Q24: **“Responsiveness first: favor predictable peer and stop latency.”** This selects how to assess the first service experiments: peer-ready delay and safe-stop responsiveness take priority over aggregate throughput; throughput costs and latency regressions remain visible. It selects no numeric SLO, hard real-time guarantee, spin policy, scheduler backend or public API.

Worker-notification and static-payload sources remain experiment candidates. All their speed-test results were removed at the user’s later request and must be redone; no peer/stop latency conclusion remains qualified.

## Source challenge and measured alternatives after Q24

The user's instruction to go beyond a rewrite authorizes experimental alternatives that preserve accepted behavior. Reports 54–57 deepen worker/controller service and async-resource lifetimes and execute two bounded Rust investigations. The catalogue reaches 537 cases and 107 tasks. Service ownership and dispatch permission are now separate glossary terms; wake tokens do not grant execution or discharge offline work.

The source alternatives remain candidates. All measured speed conclusions, raw timing stages and derived comparisons were later removed at the user’s request. Correctness models remain. Q24 governs future qualification in another suitable environment; no numerical target or production architecture is selected.

## Milestone-driven implementation authorized

On 2026-10-08 the user asked for a clearer account of progress and then instructed: “do what you are advising.” The agreed next action was a useful owned-data process, automatic sharing of one worker, safe external kill and measured peer/stop latency. This authorizes the bounded implementation experiment and replaces report-count growth as the immediate progress measure. It does not change BEAM semantics, select a production engine or close every research branch.

The [useful-buffer milestone](../experiments/process-milestone/README.md), [structured-state/helper milestone](../experiments/structured-process/README.md), [identity/lifecycle milestone](../experiments/identity-lifecycle/README.md) and [mailbox/parking milestone](../experiments/mailbox-process/README.md) pass their declared profiles. The third validates stale identities, accepted-kill arbitration and publication/shutdown ownership. The fourth adds consuming message transfer, selective receive, parking/wakeup and waiting-state cleanup. Namespace/generation exhaustion, concrete message schema and cleanup batch size remain reversible prototype policies. The [milestone tracker](milestones.md) records G1/G2/G3 as partial, with injected-clock receive timeouts next. No new user decision or architecture ADR is required for these bounded experiments. No numerical SLO or production allowance is selected.
## Machine constraint and correctness-only continuation

On 2026-10-08 the user instructed: **“NEVER do speed test / comparisions on this machine”**, explaining that concurrent work makes the results poor indicators. This overrides earlier authorization for local timing experiments. The rule is saved in the host and [project instructions](../AGENTS.md) and [context](../CONTEXT.md). No local speed test, latency/throughput benchmark, performance comparison or combined measurement runner may execute. The user subsequently ordered historical speed-test evidence deleted and redone; it is no longer retained.

The fourth milestone follows this rule: runtime clock instrumentation is absent; logical event/work/ownership assertions replace elapsed-time oracles, and bounded watchdogs only detect hangs. All 155 expected command outcomes passed, including 21 unit tests in each compiler profile, six public controls and 132 expected compilation rejections. Source-level Rust improvements include consuming payload transfer, an independent allocation receipt without a process backlink, and preserved queue nodes while saving/restoring nonmatches. These are structural results, not speed claims. The next timeout milestone will advance an injected clock explicitly.

## Injected-clock receive-timeout milestone

The user's next milestone request continued the accepted implementation sequence without changing the no-speed-test rule. The [fifth milestone](../experiments/receive-timeout/README.md) implements zero/finite receive timeouts and retains existing infinite waits. Source review confirms arming after initial scan exhaustion, preservation across unmatched wakes, timer-specific cancellation and timeout continuation at scheduler reentry. The prototype commits expiry only between polls and distinguishes clock publication from completed timer service.

The final correctness cohort passes 183 expected command outcomes: 35 unit tests per profile, inherited mailbox controls, six timeout controls, 150 expected compile rejections and eight positive shadowing executions. Earlier new cohorts remain: a formatted nested match initially failed admission; its first syntax fix omitted a checkpoint; the added logical work-count regression failed before the final fix. These are retained counterexamples and engineering corrections, not a user preference change.

Checked receive generations, weak indexed records, eager cancellation and batches of eight are reversible prototype choices. The finite numeric range follows the reference; no application exception-catching facility or real-time timer driver is claimed. No new architecture ADR is warranted. G1/G2/G3 remain partial, and monitors/Down with observer cleanup are next. Performance qualification remains prohibited on this machine.

## Performance evidence removal

On 2026-10-08 the user requested: “remove all "speed tests evidences" - they all should be redone”. This explicitly supersedes prior instructions to retain historical timing evidence. Dedicated speed runs and derived claims were deleted; mixed results were reduced to correctness records and marked as filtered. The old archive no longer includes reports/results/binaries. Source code and correctness controls remain. A new read-only audit establishes surviving artifact hashes without rerunning any test or benchmark. All speed qualification requires a new authorized run in another suitable environment.


## Sixth bounded milestone — local monitors and Down

The [monitor experiment](../experiments/monitor-process/README.md) now carries runtime Down alongside owned application messages through the checked boundary. Source review confirms the critical cancellation frontier is mailbox conversion, not target death: removing the observer origin suppresses a still-internal Down, while a queued Down requires explicit flush. Self-monitor returns a fresh reference without a relationship, and absent/stale targets report absence rather than a historical reason. These follow the accepted BEAM-default and shared-envelope decisions; no new user preference is needed.

The final [correctness cohort](../evidence/monitor-process/runs/20261008-monitor-02/results.json) passed 221 expected commands: 51 unit tests per profile, six monitor configurations, inherited mailbox/timeout controls, 178 expected compilation rejections and ten positive shadowing executions. There are 96 bounded creation/kill and 96 cancellation/kill races. A dedicated worker test observes peer execution between relationship cleanup batches using logical counters only.

Independent review identified and corrected redundant service commands for self/live monitors and retained tombstones after cancelling absent-target work. Pending entries are now removable; wakes are coalesced. Weak observer records, opaque references and concrete event variants are structural Rust opportunities, not measured speed improvements. Synthetic notifications, links/trapping, arbitrary exit terms, atomic relationship spawn and real-time timer integration remain outside this profile. G1–G4 stay partial; the next bounded milestone is local links and exit trapping. No speed test or performance comparison ran.

## Seventh bounded milestone — local links and exit trapping

The [link experiment](../experiments/link-process/README.md) extends the checked consumer boundary with local links, unlink, trap changes, genuine selectable Exit and explicit typed recovery from an absent-target link. Pinned source distinguishes direct forced kill from a propagated killed reason: only the former is untrappable. It also requires receipt-time trap classification, leaves already converted Exit events after unlink, and permits older linked exit to apply after relinking the same full process identity. The implementation preserves those distinctions without admitting general exception catching or direct exit APIs. Existing BEAM-default and shared-envelope decisions settle this scope; no new preference question or architecture ADR is needed.

The final [correctness cohort](../evidence/link-process/runs/20261008-link-02/results.json) passed 263 expected command outcomes: 68 unit tests per compiler profile, six link configurations, inherited mailbox/timeout/monitor controls, 210 expected compilation rejections and twelve positive shadowing executions. There are 96 bounded link/kill and 96 unlink/kill races. Actual-worker tests prove pending fatal exit retires without another application poll and a peer runs between cleanup batches. Deterministic tests enforce application-message → Exit → Down order and a combined eight-item link/monitor cleanup allowance. Final review strengthened the allowance test across the link-to-monitor phase transition; both passing cohorts remain retained.

The Rust representation uses one weak shared edge per process pair, removable absent-target work and coalesced service wakes. Execution classification and committed exit reason are separate so propagated reasons survive subsequent retirement. These are ownership and bookkeeping improvements by construction, not speed measurements. All performance qualification remains deferred to an authorized suitable environment. G1–G4 stay partial; atomic monitored/linked spawn is the next bounded milestone, with direct exit APIs, broader reason qualification, real-time timer integration and the room/session fixture still open.


## Eighth bounded milestone — atomic monitored/linked spawn

The [atomic-spawn experiment](../experiments/spawn-process/README.md) adds statically selected checked child functions with initial links, monitors or both. Pinned OTP creates the relationship endpoints before scheduling a local child and initializes a fresh trap flag. An accepted but unhandled kill does not mean the parent has stopped: publication can succeed while the parent is still live, and the subsequent stop reason follows its link. Successful creation need not return before the child exits. Those source facts settle the ordering without a new user decision.

The final [correctness cohort](../evidence/spawn-process/runs/20261008-spawn-02/results.json) passed 311 expected command outcomes: 83 unit tests per compiler profile, six spawn configurations, inherited mailbox/timeout/monitor/link controls, 248 expected compilation rejections and fourteen positive shadowing executions. There are 96 bounded spawn/parent-kill and 96 spawn/shutdown races. Deterministic controls force publication before and after stop/shutdown, full identity rejection, monitor-reference exhaustion and disconnected dispatch rollback. An actual-worker test observes the installed topology on its first child poll.

Review found no runtime or admission defect, but strengthened the operand-evaluation test: constant-valued helpers could conceal repeated or reordered calls. The final shadowing fixture consumes three distinct queued messages and uses an order-sensitive child result. Both passing cohorts remain retained. During development, the pending-kill unit fixture also had to consume the expected Wake command before asserting that the next command was Spawn; this corrected the fixture's protocol expectation.

Owned input moves with its existing receipt, sealed program factories admit only checked child bodies, and one publication operation closes the spawn-then-attach gap. Rejected publication removes relationship metadata before frame destruction outside the registry lock. These are structural Rust ownership and publication choices, not measured speed improvements. No new production architecture or numeric latency target is selected. G1–G4 remain partial; a bounded room/session program is next, with direct exit APIs, broader reason qualification and real-time timer integration still open. No benchmark or performance comparison ran.


## Ninth bounded milestone — integrated room and sessions

The [room/session experiment](../experiments/room-process/README.md) uses the existing checked boundary without changing runtime or compiler semantics. Two statically known session children join a room and exchange owned payloads; one fails, the other keeps communicating, and ordinary leave remains idempotent when Down follows. Each membership stores the initial full PID and monitor reference. Opposite-direction room monitors let waiting sessions settle after room death, without making the application a linked failure group.

The final [correctness cohort](../evidence/room-process/runs/20261008-room-01/results.json) passed 317 expected command outcomes. Six application configurations cover eighteen conversation, forced-room-death and host-shutdown scenarios. The fifteen-record conversation has retained expected rows and actual collector count/digest/sequence checkpoints. The cohort also passes the inherited 83 unit tests per compiler profile, 248 expected compilation rejections and fourteen positive shadowing executions. These figures are not new runtime coverage or full G5 qualification.

Review strengthened two oracles before qualification. Invalid session notifications now record a diagnostic and continue instead of ending a process, so they cannot mimic successful room-Down handling. Shutdown checks original process inputs as well as externally supplied message receipts. Saved observations come directly from collector snapshots; expected transcript data remains labeled as expected. A deliberate session failure is contained, and source ordering predicts its report before Down. No new architecture ADR or user preference question is needed for these existing BEAM contracts and reversible fixture policies.

The application reuses each owned request allocation through command, reply and collector handoffs, and duplicate membership requests do not create additional monitors. These are structural ownership/bookkeeping observations, not speed claims. The next bounded milestone is runtime-managed immutable payload sharing with zero/one/two-recipient broadcast and surviving-reader lifetime checks. General membership, a hogging session, broad resource/exit qualification and real-time timers remain open. All performance qualification remains prohibited on this machine.


## Tenth bounded milestone — immutable payload sharing

The [immutable-sharing experiment](../experiments/shared-process/README.md) implements a restricted part of accepted ADR 0008: immutable runtime-owned bytes with independently owned references across process lifetimes. Pinned BEAM source distinguishes copied heap bytes from reference-backed binaries, transfers recipient reference ownership at receive, and frees backing bytes only at the final reference. Its final off-heap cleanup loops do not themselves establish an elapsed cleanup bound. These facts settle this milestone's behavior without another preference question or architecture ADR.

The Rust representation moves a trusted host's `Vec<u8>` descriptor into private `Arc` storage, preserving its byte allocation and spare capacity. Explicit clone increments reference ownership; send moves one reference. A counters-only receipt separates physical backing capacity, storage destruction and per-holder obligations without retaining payloads. The host capacity limit is 65,536 bytes. This is a reversible restricted implementation choice; arbitrary shared Rust objects, mutable aliases, slices and authored byte construction remain outside admission. Allocation identity and source structure are evidence of avoided payload copying, not evidence of improved speed.

The [passing cohort](../evidence/shared-process/runs/20261008-shared-01/results.json) records 375 expected command outcomes, 340 observations and 185 preserved inputs. Both profiles pass 95 unit tests, 148 compiler rejections and eight positive shadowing examples. Six sharing configurations cover sixty bounded broadcast/lifetime scenarios and 96 shared-send/kill races. Actual checksum results qualify readable survival after publisher and receiver death; quiescent receipts qualify independent references and exactly-once final release. Existing room controls and prior source/evidence are preserved.

Review added per-child acknowledgement-before-Down assertions and cancellation inside a suspended cloned send operand. Partial fanout cancellation is explicitly a reference-accounting oracle; it does not inspect that child's checksum, which is covered by separate known-handle fixtures. During development the checker correctly rejected unsupported `-=` syntax; the application uses admitted subtraction plus assignment. The final source review found no remaining concrete defect, and the first retained cohort passed. No speed test or performance comparison ran.

The next bounded milestone moves sustained admitted computation into a session of the room/broadcast fixture. The chosen priority remains predictable peer and stop responsiveness; local qualification uses logical progress and cancellation opportunities only. General membership, broader fanout/death interleavings, direct exit APIs, real-time timers and full G1–G5 qualification remain open.


## Eleventh bounded milestone — busy room member

The [busy-member experiment](../experiments/busy-session/README.md) exercises the accepted responsiveness priority through logical progress and cancellation observations. Pinned BEAM source places reduction checks on dispatch, handles fatal signals independently of application receive, and separates monitor notification from final off-heap reclamation. Those facts supply the behavioral obligations; they do not imply a numeric latency target for generated native Rust. No new user preference or architecture ADR was needed.

The room monitors a host-created busy member, spawns a healthy member and broadcasts the same immutable bytes to both. The busy member computes in nested admitted helpers without another receive, handwritten yield or stop check. Room/healthy-peer exchanges complete before direct kill, fatal linked exit or active shutdown. Explicit-stop cases require exact Down and two further survivor replies; shutdown separately settles all actors and ownership without assuming post-shutdown application notification consumption.

The [first retained cohort](../evidence/busy-session/runs/20261008-busy-01/results.json) passed 381 expected command outcomes, retaining 346 observations and 186 inputs. Six configurations cover eighteen new application scenarios; 95 unit tests per compiler profile, 148 rejections per profile and 8 positive shadowing examples per profile are inherited. Eighteen runtime/compiler files match the previous milestone after crate renaming. Prior source/evidence remains preserved.

The busy member releases its held share and nineteen queued shares, plus three ordinary messages, in the existing three queue-cleanup batches. Independent room, healthy-member and host references remain valid. Final process cleanup leaves the host able to read all bytes; its last drop releases the backing once. Moving Work/Reader records through helpers does not create additional byte references: the original 23 created references remain constant during continued computation and replies. This is a Rust ownership result, not a measured speed improvement.

Review strengthened the busy computation oracle with checksum checks after stopped cleanup, avoiding reliance on a simultaneous snapshot of live atomics. The checker rejected an unsupported division during development; the fixture now uses admitted addition/subtraction. Source review found no remaining concrete correctness defect. No speed test or performance comparison ran, and no production allowance or elapsed-time bound was selected.

The next bounded milestone replaces a stopped session in one room slot and isolates its stale messages, identities and monitor notifications from the new member. Shared ownership must survive replacement and settle on shutdown. General membership, richer exits, real-time timers and complete G1–G5 qualification remain open.


## Twelfth bounded milestone — session replacement

The [replacement experiment](../experiments/session-replacement/README.md) composes accepted full-identity, monitor, selective-receive and immutable-sharing contracts. Pinned OTP lookup indexes the table and then checks the complete PID; an absent old identity discards ordinary sends. Monitors retain their own references and the exiting incarnation's identity. OTP advances masked PID data on slot release and can eventually wrap; Bream's checked generation exhaustion retires slots without wrapping. The fixture does not confuse these different guarantees.

A room and two members fill a three-slot registry. Once the old member emits its report and exits, its slot is the only one available to the replacement. The host requires the report and genuine Down already queued before replacement input arrives. A further retired-epoch request remains queued as well. New readiness is selected across these old events; later consumption accounts the retired record without changing current membership. Old-PID Words/shared sends discard their obligations; plain old-reference cancellation preserves queued Down and the new monitor. New and surviving members then return valid payload checksums.

Ordinary application envelopes have no authenticated sender field. The scalar epoch is a trusted room protocol label, not a runtime authority or safe basis for receive-prefix skipping. The timing case is deferred consumption of already published Down, not delayed runtime publication. Acknowledgement contents at final close are not separately inspected; exact child Down and prior checksum probes carry the relevant oracles. These explicit limits avoid extending the milestone beyond its checked evidence or adding a new product/API decision.

The [first retained cohort](../evidence/session-replacement/runs/20261008-replacement-01/results.json) passed 387 expected command outcomes with 352 observations and 187 inputs. Six configurations cover twenty-four normal/failure replacement, room-kill and active-shutdown scenarios. Each profile retains 95 runtime tests, 148 expected compiler rejections and eight positive shadowing examples. Eighteen runtime/compiler files match the preceding milestone after crate renaming; prior inputs and evidence remain preserved.

Owning helper state moves without cloning the shared handle. Six references account for each continuing conversation; teardown adds nineteen queued shares for twenty-five total. After all process cleanup the host still reads the bytes, then its final drop frees the backing once. Three old events plus nineteen queued shares retire in three queue-cleanup batches. These are ownership and operation-count results, not elapsed-time or speed evidence.

Source review found no blocking correctness gap. Development corrected use of the host-only demonitor_info API and a bool expectation for the checked unit-returning demonitor; final observations assert queue preservation and current monitor behavior instead. No runtime behavior changed, no speed test ran and no new architecture ADR was needed. A freshness-proved receive marker remains a future optimization under PERF-014; arbitrary scalar epochs cannot establish that proof.

The next bounded milestone adds checked self-exit and targeted local exit for restricted reason values, distinguishing explicit kill from ordinary killed propagation. It must preserve trapping, exact Down/Exit behavior, stopped execution and shared cleanup. General membership churn, arbitrary reasons, real-time timers and full G1–G5 qualification remain open.
