# Bream build plan: native BEAM-style process primitives

**Planning baseline: 2026-10-07; latest update: 2026-10-09. Full feature qualification remains open.** This is the build and research inventory for the accepted first product and explicitly conditional extensions: an ordinary Rust dependency providing automatically scheduled, safely terminable native processes. It organizes the known branches, unresolved mappings, implementation work and acceptance evidence. It does not claim that every possible interleaving or future feature has already been specified.

The [milestone status](milestones.md) tracks implementation separately from inventory counts. Fourteen restricted profiles now pass: [useful buffers](../experiments/process-milestone/README.md), [structured state/helpers](../experiments/structured-process/README.md), [identity/lifecycle](../experiments/identity-lifecycle/README.md), [owned mailboxes/parking](../experiments/mailbox-process/README.md), [receive timeouts](../experiments/receive-timeout/README.md), [local monitors/Down](../experiments/monitor-process/README.md), [local links/exit trapping](../experiments/link-process/README.md), [atomic monitored/linked spawn](../experiments/spawn-process/README.md), [bounded room/session integration](../experiments/room-process/README.md), [immutable payload sharing/broadcast](../experiments/shared-process/README.md), [busy room-member integration](../experiments/busy-session/README.md), [bounded session replacement](../experiments/session-replacement/README.md), [checked self-exit/targeted local exit](../experiments/explicit-exit/README.md) and [runtime-owned local reads](../experiments/owned-read/README.md). The latest qualifies one-shot external read ownership across actor stop, exact-incarnation completion, bounded pending/ready admission and host shutdown. G1–G5 remain partial; no complete feature group is qualified by these results. Bounded sequential reads on a persistent endpoint are next; general membership, broader exit semantics and a real-time timer driver remain open. The user's [machine constraint](../AGENTS.md) prohibits all local speed tests and performance comparisons; historical local timings cannot qualify performance.

The reference is Erlang/OTP `OTP-29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. [Source provenance](../evidence/beam-source-review/source-pin.json), [source map](beam-source-map.md), [behavior coverage audit](behavior-coverage.md). Before asking a behavior question, inspect the reference: clear BEAM behavior is the default to reproduce. An unresolved implementation detail is engineering work, not a request for the user to redesign BEAM.

This plan is the detailed feature/case inventory. The [roadmap](roadmap.md) supplies experiment rationale; [ADRs](adr/) record accepted decisions; the [design tree](design-tree.md) preserves the interview. The production execution engine, concrete public signatures and complete admitted Rust surface remain open. The generated-process model is an accepted research route, not a selected production architecture.

## Read the plan by workstream

| Workstream | Features | Branch cases | Detailed plan |
|---|---:|---:|---|
| Reference, checked code, integration and measurement | 7 | 60 | [Admission and conformance](build-plan/admission-and-conformance.md) |
| Scheduling, resources, failure, embedding and scaling | 7 | 106 | [Execution and resources](build-plan/execution-and-resources.md) |
| Sending, selective receive, timeouts and internal signals | 4 | 51 | [Messaging](build-plan/messaging.md) |
| Identity, monitors, links, exits and reclamation | 5 | 86 | [Lifecycle](build-plan/lifecycle.md) |
| Deeper process-runtime branches and conditional extensions | 17 | 234 | [Process-runtime expansion](build-plan/process-runtime-expansion.md) |
| **Total** | **40** | **537** | [Machine-readable feature index](build-plan/features.json) |

Every case has a stable ID, such as `LIFE-02.C05`, so future issues, fixtures and results can refer to the same branch. Cases can contain a matrix or a race family: 537 cases is an inventory count, not a count of implemented tests or a completeness percentage. Add new IDs when new branches are found; retain old IDs when correcting their contract. The original 23-group/199-case catalogue is preserved. Successive source passes added 138, 29, 26, 31, 24 and 27 cases; the concurrency/clock pass added 26; the task/code/routing pass added 23; the worker/async-service pass adds 14 to existing groups. Selected reference variants now have executable observations, while no Bream feature is qualified. Conditional optimizations, later capabilities and representation-specific reference research are labelled and do not silently become initial-release requirements. The [expanded source inventory](process-runtime-inventory.md) explains the process boundary and successive source/experiment reports; the [current documentation/evidence validation](../evidence/worker-service-review/review-validation.json) checks the catalogue, not runtime behavior.

## Performance work alongside the feature gates

The [performance backlog](performance-plan.md) contains **107 planned experiments**, separately indexed in [performance-tasks.json](build-plan/performance-tasks.json). Each specifies a baseline/treatment, feature gates, workload, measurements and acceptance/rejection conditions. Reports 46–49 add 26 cases on reclamation participation, process-lock ownership, clock/deadline semantics and weak-memory qualification, plus 15 performance tasks. Reports 50–53 add 23 cases and 14 performance tasks on system-work settlement, code/literal lifetime and typed routing; targeted naming/variance observations are separately executed. Reports 54–57 add 14 cases and eight planned tasks. All isolated ownership/notification speed evidence has been removed and requires new qualification; correctness models and source remain. The user selected responsiveness before throughput for the first service experiments. [Report49](research/49-rust-atomic-protocol-observations.md) separately executes three small Loom protocol families: three positive variants complete and four deliberately weakened controls fail as expected. These are not Bream runtime qualification; earlier Linux, language, VM and model evidence remains separately recorded.

Continue source-level cost analysis, ownership proofs and bounded correctness experiments here. Defer all timing-based work below, including G6, to a separate suitable environment with explicit authorization. Checkpoint/live-state costs, owned payload transfer and room fanout remain priorities for that future qualification; timer and multi-worker optimizations stay behind their feature gates. Preserve scheduling, forced-stop and ownership contracts while investigating alternatives. The [context working rule](../CONTEXT.md) makes this review part of future branch investigations.

The [Rust opportunity map](rust-opportunities.md) explains which dynamic representation and bookkeeping work typed ownership might remove, with remaining runtime obligations explicit. [Language-probe evidence](research/41-rust-guarantees-and-process-admission.md) records 24 compilations and 12 executions, including intentional acceptance counterexamples for overly broad admission bounds.

## Product boundary

The first qualification target is Linux x86-64, one scheduler worker, native Rust through ordinary Cargo and standard rustc. Admitted CPU-heavy code must lose and regain execution automatically, with no handwritten yield, await or cancellation check required for fairness. Processes own state and messages; shared data is limited to accepted runtime-managed immutable byte storage. Forced kill must stop application execution safely and permit reclamation of covered resources.

The first user-facing facilities are process creation/identity, owned typed messages, one growing mailbox per process, selective receive and timeout, monitors, links, exit reasons, trapping and kill. Application payloads and eligible lifecycle notifications share one selectable runtime envelope. Internal control must work independently of application receive. The integration example is raw room/session processes, without GenServer, supervisors or automatic restart.

One worker is a qualification stage, not a final scalability claim. Runtime-aware I/O and synchronization are acceptable, but a socket driver or arbitrary host-native work is not needed to establish this first process slice. [BEAM versus OTP boundary](research/10-beam-versus-otp-boundary.md).

Bundled utility algorithms such as ZIP, compression and cryptography are excluded from the port. Rust libraries can supply those operations; any call admitted inside a guaranteed process still needs a scheduling and resource-lifetime contract. OTP behaviours remain consumers of these primitives.

The following distinctions remain explicit:

- A turn allowance is renewable. Exhausting it suspends a computation; it does not consume a lifetime quota or kill the process.
- Ordinary local PID send does not wait for consumption or mailbox capacity, and a dead PID destination discards the message. Later named-address forms retain their separate missing-name rules. Acceptance does not promise application handling or infinite host memory.
- Safe termination covers the admitted execution/resource model. An opaque native call is not made safely killable by moving it to another worker.
- `Down` requires stopped application execution and removal from live lookup. Tracked cleanup may finish later. Other processes may retain shared immutable bytes after that cleanup completes.
- BEAM determines observable behavior. Its term representation, garbage collector, queue layout and numerical reduction constants are not mandatory Rust implementation choices.
- Typed protocols do not settle every Rust mapping. Catchable failures, notification look-alikes, payload transfer, panic behavior and host-runtime lifetime still need explicit contracts.

## Feature catalogue

All entries start in `planned` state. A feature becomes qualified only when its applicable case contracts, Rust mapping, implementation and evidence satisfy the completion rule below. “Start” refers to the gate at which meaningful work begins, not a requirement to finish the entire feature in isolation.

| ID | Deliverable | Start | Main dependency or integration |
|---|---|---|---|
| REF-01 | Versioned operation contracts and allowed-outcome catalogue | G0 | Pinned docs/source/tests; extended by every feature |
| REF-02 | Reference fixtures, differential checks, bounded interleavings and fault injection | G0 | REF-01 expected outcomes; implementation adapters added incrementally |
| LANG-02 | Useful owned types, protocols, errors and operations | G0 | REF-01; room/session source sketch; resource inventory |
| LANG-01 | Enforceable public process-admission boundary | G1 | LANG-02 proposed surface; external consuming-crate bypass tests |
| LANG-03 | Automatic suspension and resumable approved helpers | G1 | LANG-01/02; cancellation-safe state representation |
| EXEC-01 | Scheduler turns, exclusive execution, parking and wakeup | G2 | LANG-01/03; LIFE-01/05 ownership and terminal arbitration |
| EXEC-02 | Work accounting and schedulable runtime operations | G1–G2 | LANG-02/03, EXEC-01; extended for each primitive |
| EXEC-03 | Resource accounting, immutable buffers and phased cleanup | G1–G2 | LANG-02/03; send, death and reclamation ownership |
| EXEC-04 | Panic, allocation, capacity and fatal-failure contract | G0–G2 | Every allocation/publication site; EXEC-03 cleanup |
| EXEC-05 | Embedded-runtime construction, ownership and shutdown | G2 | EXEC-01/03/04; identity and stop-before-free contract |
| EXEC-06 | Runtime inspection, trace events and accounting | G2 | Instrument each new primitive; APP-01/MEASURE-01 consume it |
| LIFE-01 | Spawn, incarnation identity and atomic initial relationships | G2 | EXEC-01; relationship hooks finished at G4 |
| LIFE-05 | Forced stop, death observation and reclamation | G2 | EXEC-01/02/03; wake/control and relationship integration at G3/G4 |
| MSG-04 | Internal signal transport, causal order and budgets | G2–G3 | LIFE-01/05, EXEC-01/02/03; relationship signal types at G4 |
| MSG-01 | Typed envelope, sending and payload ownership | G3 | LANG-02, LIFE-01, MSG-04, EXEC-03/04/05 |
| MSG-02 | Selective receive, guards and saved scan state | G3 | MSG-01, LANG-03, EXEC-02; exception/exit mapping |
| MSG-03 | Receive timeouts and publication/wakeup handshake | G3 | MSG-02, EXEC-01/03/05, LIFE-05 |
| LIFE-04 | Exit operations, reasons and receipt-time trapping | G4 | LIFE-05, MSG-01/04; catchable error versus runtime control |
| LIFE-02 | Monitors, demonitor, flush and info | G4 | LIFE-01/05, MSG-01/02/04; endpoint ownership |
| LIFE-03 | Links, unlink/relink and exit propagation | G4 | LIFE-01/04/05, MSG-04; correlated removal protocol |
| APP-01 | Raw room/session acceptance fixture | G0 sketch; G5 execution | Qualified initial primitives, failures, shutdown and inspection |
| MEASURE-01 | Reproducible density, fairness and cleanup cost report | G6 | Correct APP-01, EXEC-06 and retained REF-02 evidence |
| EXEC-07 | Multiple workers, migration and concurrent reclamation | G7, later | Correct single-worker slice; transfer proof and race harness |

The table above retains the original 23 groups. The [expansion catalogue](build-plan/process-runtime-expansion.md) adds EXEC-08, MEM-01, STATE-01, MSG-05–07, TIME-01, LIFE-06–07, NAME-01, OBS-01–02, IO-01, LANG-04, CODE-01, SIG-01 and RES-01 with their applicability, work, dependencies and cases. Initial ownership, exception, inspection and late-control findings feed the existing gates; extensions remain separately conditional.

The detailed documents list dependencies as integration obligations. Some are intentionally mutual: send, death and cleanup share one ownership protocol; links and exits share one signal state machine. Do not interpret these as a circular instruction to finish every full API before starting any of them. The gates below define incremental slices, starting with models and shared invariants, then adding observable operations.

## Build order and exit gates

### G0 — Establish contracts and the useful-program sketch

Start REF-01/02, LANG-02 and the failure classification in EXEC-04. Write one Rust room/session source sketch and equivalent raw Erlang/Elixir scenarios. Label every proposed Rust operation and type; snippets are not advertised as compiling until they do. Establish a test harness that records the exact reference executable identity, scheduler options, fixtures and raw observations.

The [pinned reference observations](research/29-pinned-reference-observations.md) now execute synthetic notification flush, a defined local EXIT/DOWN/priority setup, dynamic timeout validation and additional boundary fixtures on OTP29.1.1/ERTS17.1. Preserve their exact assumptions; broaden the required race/option products and resolve Rust representation conflicts. G0 is not complete merely because these samples ran. The earlier host OTP29/ERTS17.0.5 is not the reference executable; build provenance and hashes identify the actual tested binaries.

**Exit evidence:** a versioned initial operation profile; allowed-result specifications for the first fixtures; an operation/type/cleanup inventory for the sketch; explicit unresolved mappings with case IDs. Unresolved questions about one operation block that operation, not unrelated research. Do not fabricate exact answers to close a checklist.

### G1 — Prove a useful public execution boundary

Implement LANG-01/03 against LANG-02's proposed surface. Start resumable operations and ownership checks in EXEC-02/03. Compile programs through a separate consumer crate; exercise bypasses as well as supported state and helper calls. Cover loops, supported recursion, operators, matching, copies, collections, partial initialization and cleanup.

**Exit evidence:** useful state and computation compile through the intended public boundary, no handwritten scheduling checks are required, and ordinary safe API construction cannot smuggle unqualified execution or destruction into that boundary. Retain the admission/ownership argument, positive runs, rejection diagnostics and release-build behavior. The old closed arithmetic/buffer probe is insufficient.

**Failure gate:** if the ordinary-Cargo approach cannot enforce a useful model, document the concrete blocker and alternatives before extending the primitive API. Do not trade away automatic scheduling or safe kill to preserve a prototype.

### G2 — Establish single-worker execution, identity and stop

Build EXEC-01, LIFE-01's registry, LIFE-05's terminal transition, basic internal control, tracked cleanup and host lifetime. Start inspection here. Integrate capacity/allocation/panic contracts before relying on their recovery behavior. Primitive models may use synthetic signals; initial monitor/link hooks must be part of the publication design even though their full public operations arrive at G4.

**Exit evidence:** a hog repeatedly suspends/resumes while a peer progresses; kill works when running, queued or waiting; no application resume follows committed death; no active continuation is freed; slot reuse and stale wakeups are safe; tracked state is released exactly once; shutdown and partial initialization have defined failure paths. Keep kill request, stop, identity removal, notification and reclamation separate in traces. `Down` itself is qualified when monitors are added.

### G3 — Add mailbox ownership, selective receive and timers

Complete the first MSG-04 transport slice, then MSG-01/02/03. Implement the send/receive/death ownership table before optimization. Start with simple physical queues and unoptimized scanning; do not require fresh-reference markers or queue batching to establish behavior.

**Exit evidence:** message-major selection and clause order, retained unmatched messages, scoped cursor reset, no lost wakeups, timeout evaluation/arming/cancellation semantics and stale-event rejection. Kill at each preparation/publication/scan/wait transition. Large scans, admitted guards, copying and queue cleanup must participate in automatic scheduling. Received payloads survive sender death and shared storage survives while recipients own it.

### G4 — Complete lifecycle relationships and exit semantics

Implement LIFE-04's transition table, LIFE-02 monitors and LIFE-03 links over the transport, then finish atomic monitored/linked spawn. Qualify catchable absent-local-link errors, self operations, repeat operations, cancellation barriers, queued notifications, flush/info, unlink acknowledgments, trap-flag changes and competing exits. Bring all notification producers into the same selectable envelope.

**Exit evidence:** the full applicable exit matrix and bounded relationship interleavings satisfy their contracts; monitor observations obey stop-before-`Down`; late messages/control carry valid ownership; combined relationship behavior has a reference-backed allowed trace. Do not use “kill always wins,” global ordering or a single passing stress run as shortcuts.

### G5 — Exercise the raw room/session program

Run APP-01 on the qualified single-worker slice. Exercise joins/leaves, duplicate membership requests, broadcast, selective replies, session/room death, retained shared payloads, a separate hog and a hogging session. Use monitors for application membership cleanup and a separate linked pair for link behavior; do not turn the fixture into an implicit supervision framework.

**Exit evidence:** all application policies are explicit and distinguished from runtime semantics; peers progress; death and resource accounting remain correct during traffic; inspected state is truthful; failures and host shutdown complete according to their contracts. Payload parsing/transformation is a proposed useful CPU fixture, not an already-selected user algorithm. Real networking remains a later integration.

### G6 — Measure the correct single-worker slice

Run MEASURE-01 with raw results and reproducible build/environment metadata. Measure idle and populated process costs, mailbox growth, immutable sharing, throughput, scheduling delay, kill-to-stop and stop-to-reclamation separately. Include helpers, internal signals and reclamation in load scenarios.

**Exit evidence:** an operating-range report with comparable state/protocols and honest limits. Measured maxima do not establish hard deadlines; tiny future-object size does not establish tiny process memory. Do not select product thresholds or advertise arbitrary Rust compatibility from one arithmetic benchmark.

### G7 — Qualify multiple workers, then extend capabilities

EXEC-07 adds concurrent claims, sleep/wakeup, transfer, queue ownership and reclamation. Prove transfer eligibility before migration and preserve every applicable single-worker case. Repeat affected measurements only after correctness checks.

**Exit evidence:** bounded interleavings and native stress establish the stated invariants within their documented limits; no simultaneous execution, stale retargeting, lost work or premature free. Worker failure and thread-affine state have explicit contracts. This gate is later work and cannot rescue a failing one-worker admission or termination model.

## Cross-feature invariants and branch axes

These obligations apply to every relevant table, even where a row is not repeated in every document:

1. One application execution owner per incarnation; no application resume after the terminal transition.
2. Every continuation, message, endpoint, timer and immutable-buffer reference has accountable ownership across preparation, publication, suspension, death and reclamation.
3. A full identity and a safe record lifetime are separate requirements. Slot/generation comparison alone is not a memory-safety proof; runtime instances and eventual identifier reuse need explicit treatment.
4. Same-sender causal ordering is preserved where BEAM specifies it. Independent senders may interleave; control processing does not require application selection.
5. Each receive selects at most one message. Rejected messages remain owned and reachable, timeout cannot complete a later receive, and waiting cannot lose both work and its wakeup.
6. Direct forced kill is runtime control. Trapping, application catch and a mailbox pattern cannot neutralize it. A committed competing termination can still determine the final reason under the reference race contract.
7. `Down` certifies death under Bream's accepted stronger stop rule, not globally freed storage. Cleanup completion accounts for the dead process's obligations without invalidating other owners.
8. Failure handling must preserve the same ownership invariants as success. Catchable operation errors, process exit, panic unwinding and host-fatal failures are not interchangeable.

Expand cases over applicable axes: self/other; live/exiting/dead/reused identity; published/unpublished/selected state; running/ready/waiting; active/cancelling/removed relationship; direct/linked signal; normal/kill/killed/other reason; trapping/untrapped; empty/large mailbox; before/after an ownership transfer; and recoverable failure/host-fatal boundary. Specify which combinations are meaningful before generating a Cartesian product. Repeated observations of one schedule do not cover the other branches.

## Evidence and definition of done

For each future executable case, retain these fields independently:

| Field | Required content |
|---|---|
| Identity/scope | Stable case ID, feature, version, supported operation/options and conditional applicability |
| Setup | Process states, ownership, relationships, mailbox contents and scheduler configuration |
| Events | Inputs, explicit happens-before relations and intentionally concurrent events |
| Oracle | Required results or allowed set, forbidden observations, reference source/test and guarantee versus implementation-detail label |
| Rust mapping | Public operation, errors/reasons, payload effects and cleanup/observation points |
| Reference evidence | Not run / run with exact executable identity; raw result, fixture and environment |
| Bream evidence | Not implemented / implemented / executed result; compiler flags, artifact identity and raw trace |
| Model evidence | Not explored / explored; state bounds, abstraction assumptions and counterexamples |
| Remaining obligations | Unresolved mapping, unsupported variant, ownership/soundness argument or measurement still needed |

A feature is qualified for a declared profile only after its applicable cases have resolved contracts, implementation checks and appropriate evidence, including failure and forbidden outcomes. Model exploration and testing support an ownership/soundness argument; neither proves all programs safe on its own. Deferred cases remain visibly deferred, not passed. The plan's JSON index records **planned** case status and never stands in for test output.

The earlier two Bream mechanism probes remain valid for their narrow inputs. [Report29](research/29-pinned-reference-observations.md) adds targeted BEAM reference observations, including a reproduced debug assertion; it does not implement Bream or run the complete upstream suite. Documentation checks verify IDs, links, source ranges and artifact consistency, not arbitrary program behavior.

[Report 33](research/33-finite-ownership-interleavings.md) adds two executed finite models for retained-record handoff and direct-message parking. Their baseline graphs exhaust under explicit sequential-consistency assumptions; five broken variants produce counterexamples. This is partial model evidence for LIFE-05, EXEC-01 and MSG-03, not completed interleaving qualification. Buffered ingress, timer/control products, multiple workers and actual Rust ownership remain outside these models.

### Next protocol experiments

| Experiment | Concrete controlled boundary | Required evidence |
|---|---|---|
| Buffered publication and retirement | Pause sender after array acquisition, before slot lock and after append; close each slot separately during receiver exit. | Account Process, array and signal ownership independently; include first-arrival wakeup and MSG-04.C09–C16. Preserve accepted signal order and forbid post-death application execution. |
| Late buffer installation candidate | Pause a retained control sender until exit has deinstalled buffers, then permit its fallback in release/debug configurations. | Establish reachability and replacement-array ownership for MSG-04.C17. A source suspicion is not an observed leak; keep instrumented builds separate from the untouched reference. |
| Relationship cancellation products | Pause before notification conversion, at pending MONITOR flush, and around unlink/relink ACK comparison. | Enumerate permission/ownership outcomes, distinguish genuine DOWN from application look-alikes, and prove which internal branch a fixture reached. Ordinary public-API barriers alone may not force the desired race. |

These extend the reference/model work. G1's useful public Rust admission experiment remains a separate decisive gate; protocol findings do not make opaque application code safely preemptible or killable.

## Blocking mappings to settle before the affected API

| Mapping/evidence gap | Owning cases | Concrete next artifact |
|---|---|---|
| Public Rust admission and transitive helper coverage | LANG-01, LANG-03, EXEC-02 | Consumer-crate positive/bypass fixtures and ownership argument |
| Catchable errors, process exceptions, signals and Rust panics | LANG-04, LANG-02.C09, LIFE-03.C03, LIFE-04.C02/C09–11, EXEC-04 | Reference error table plus proposed Rust representation and negative controls |
| Lifecycle-shaped application data and synthetic flush discrepancy | REF-01.C03/C04, LANG-02.C08, MSG-01.C07, LIFE-02.C12 | Use report29’s executed synthetic-tuple control; settle explicit typed-envelope mapping and remaining variants |
| Timeout validation and arrival/expiry ordering | MSG-02.C06, MSG-03.C04/C06/C07 | Ordered/racing reference fixtures and receive transition model |
| Combined relationships, unlink/relink and competing exits | LIFE-01.C05, LIFE-03.C05/C09/C10, LIFE-05.C05 | Partial-order oracle plus bounded lifecycle model |
| Resource release, last-owner cleanup and completion | EXEC-03, LIFE-05, MSG-04.C07 | Resource inventory and per-transition ownership/reclamation table |
| Embedded runtime handles, shutdown and foreign instances | EXEC-05, LIFE-01.C09, MSG-01.C05 | Host ownership model, admission cutoff and shutdown race cases |
| Capacity, allocation and fatal boundaries | EXEC-04 | Failure-site catalogue, supported build assumptions and fault-injection plan |

These are research/design tasks. Ask the user only when evidence reveals a genuine ambiguity or conflict with accepted requirements, accompanied by the concrete reference behavior and proposed adaptation.

## Explicit later backlog

The deeper review now gives many of these facilities named groups and branch cases in the 40-group inventory. They remain conditional or later scope; their presence is not a claim of complete specification. The table names further work beyond the traced branches. A shared-table or dynamic-code proposal still needs its own Rust ownership/admission contract before implementation.

| Facility | Branches its future plan must cover |
|---|---|
| Runtime-aware socket/file I/O | Readiness versus completion, cancellation versus completion, partial operations, native buffer lifetime, registration reuse, owner death, shutdown, error delivery and scheduler accounting |
| Additional synchronization | Waiter registration, wake/timeout/kill races, owner death and cancellation-safe cleanup; no implicit admission of arbitrary shared mutable application state |
| Names and aliases — NAME-01, MSG-05 | Complete the typed-address mapping and interleaving oracles for the traced registration, reuse and deactivation branches |
| General timers — TIME-01 | Execute expiry/cancel/read races, recipient-death cleanup and paused-reference continuity; receive timers are already initial scope |
| Priority scheduling/messages | Per-priority order, starvation behavior, priority acceptance and interactions with selective receive and lifecycle signals |
| Receive markers and transport optimizations | Freshness proof, invalidation, death/timeout cleanup, batching and equivalent outcomes against the unoptimized implementation |
| External host-native jobs | Separate job ownership, native work outliving a caller and resource retention; never weaken the accepted process stop-before-Down guarantee |
| Distribution | Node identity, disconnect/reconnect, remote sends/links/monitors, ordering, serialization, resource limits and failure reasons |
| ETS-like shared tables — RES-01 | Qualify traced transfer/heir/deletion lifetimes only if an explicit shared-state design is accepted; table/query algorithms are outside this audit |
| Code loading/upgrades — CODE-01 | A native continuation/code lifetime design, state migration and complete purge/inspection races; current research traces only the process-facing boundary |
| Other platforms | Interruption/clock/I/O backends and native ABI/ownership qualification per platform |
| OTP behaviours | Separate libraries consuming primitives: server conventions, supervisors and restart policies; explicitly outside this runtime build plan |

The immediate next implementation work extends the passed one-shot read into bounded sequential reads on a persistent endpoint. The [milestone tracker](milestones.md) gives its endpoint/request ownership, stale-operation, stop and shutdown exit conditions. Runtime-owned read storage, checked self-exit and targeted local exit have passed their restricted profiles. Broader G0–G5 admission/reference obligations, general membership, arbitrary reasons and real-time timer integration remain open; the inventory does not imply that safety or conformance is already established.
