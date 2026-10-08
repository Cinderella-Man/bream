# Admission, reference tests and the room fixture

Part of the [Bream build plan](../build-plan.md). Full feature/case qualification remains **planned**. The [useful-process milestone](../../experiments/process-milestone/README.md) now implements and tests a restricted public buffer/primitive profile; it does not complete these broader cases. Behavior follows the pinned BEAM reference; implementation sketches and unresolved Rust mappings are labelled separately. Case IDs remain stable.

## REF-01 — Reference contract and case catalogue

**Purpose:** specify the supported ordinary local-process profile before advertising BEAM compatibility. Dependencies: none; grows with every primitive. Inputs: [coverage audit](../behavior-coverage.md), [mailbox audit](../research/15-mailbox-edge-cases.md), [lifecycle audit](../research/16-lifecycle-edge-cases.md), [execution/resource audit](../research/17-execution-resource-edge-cases.md).

Build tasks:

- Record the reference release/commit, interpreter/JIT mode and applicable options with every reference result.
- Catalogue each public operation's success, no-op, error, asynchronous consequence and resource transitions.
- Separate documented guarantees, pinned implementation observations, source-predicted discrepancies and unresolved Rust mappings.
- Express races as partial orders plus allowed outcomes; do not require one observed total trace.
- Map reference failure classes and exit reasons into the proposed Rust surface before the corresponding operation is considered specified.

| Case | Branch and required treatment | Acceptance evidence / forbidden shortcut |
|---|---|---|
| REF-01.C01 | One documented result follows from ordered events. | Fixture has exact inputs, result and forbidden observations; primary reference attached. |
| REF-01.C02 | Independent concurrent events permit several results. | Oracle describes the legal set and its ordering assumptions; never promote one sampled winner into the contract. |
| REF-01.C03 | Documentation and inspected code differ or leave a corner ambiguous. | Retain both sources and a minimal exact-version fixture; mark unresolved until reconciled. Synthetic-tuple demonitor flush is the first example. |
| REF-01.C04 | The reference uses Erlang terms/exceptions where Rust has typed values. | Specify the mapping and any observable difference; typed representation does not excuse silently changing failure propagation. |
| REF-01.C05 | A BEAM facility is outside the selected first scope. | Mark unsupported/deferred explicitly; do not count it as implemented or as a hidden acceptance dependency. |
| REF-01.C06 | A reference or implementation test was only inspected. | Mark inspected, not executed/passed. Source coverage and test coverage remain separate. |
| REF-01.C07 | A newer reference changes behavior. | Preserve old evidence and assess the compatibility difference before changing the selected baseline. |

**Done when:** every exposed initial operation has a versioned case table and Rust mapping, and every unresolved branch is a visible gate rather than an assumed default. This is not a claim to enumerate every possible program.

## LANG-01 — Public process admission boundary

**Purpose:** ordinary safe construction cannot label arbitrary uninstrumented Rust computation as a guaranteed Bream process. Dependencies: LANG-02's proposed surface; REF-01. The original probe lacks this boundary; the [new separate-consumer milestone](../../experiments/process-milestone/README.md) now tests an opaque Program and checked buffer/primitive profile. Broader operation/type qualification remains open. [Generated-model investigation](../generated-process-model.md).

Build tasks:

- Design the generated program representation and runtime entry point together; exact public types/macros are still experimental.
- Put positive examples and attempted bypasses in a separate consuming crate with only the intended public dependency.
- Audit helper visibility, construction, trait implementations, reexports and macro hygiene; privacy alone is not a transitive execution proof.
- Define what is rejected with useful compile diagnostics. Fail closed when the checker cannot establish the required contract.

| Case | Branch and required outcome | Acceptance evidence / forbidden outcome |
|---|---|---|
| LANG-01.C01 | A supported program and supported initial state are constructed by the intended API. | Compiles and runs without handwritten scheduling yields; guarantees attach to this admitted representation. |
| LANG-01.C02 | A caller supplies an arbitrary `Future`, closure or function pointer. | Cannot enter the guaranteed path merely by matching an ordinary Rust type or trait signature. |
| LANG-01.C03 | A caller constructs/implements a marker or invokes public generated-code helpers directly. | Cannot forge admission through safe API access; externally compiled negative fixtures demonstrate the boundary. |
| LANG-01.C04 | Raw identifiers, shadowed names, imports, reexports or macro expansion disguise an unsupported operation. | Resolve/reject safely; no textual-name allowlist that silently accepts different code. |
| LANG-01.C05 | A trait operator, callback, conversion or custom destructor hides arbitrary work. | Reject or qualify the invoked implementation transitively; `Send + 'static` is insufficient. |
| LANG-01.C06 | State reaches globals, TLS, scoped threads, locks, FFI or unapproved dependencies. | Reject unsupported access in guaranteed process code; host capabilities do not automatically inherit process guarantees. |
| LANG-01.C07 | Accepted syntax reaches an unhandled checker branch or compiler-generated path. | Clear rejection or a demonstrated safe lowering, never best-effort acceptance. |
| LANG-01.C08 | Host `unsafe` code violates Rust invariants or corrupts runtime storage. | Do not claim an in-process hostile-code security boundary. The guarantee's scope must be explicit, not falsely extended from safe admission tests. |

**Gate:** if ordinary Cargo/proc-macro tooling cannot enforce a useful boundary, stop extending the primitive API and document the concrete failure. Do not silently weaken preemption or safe termination to keep the prototype progressing.

## LANG-02 — Useful owned state and data operations

**Purpose:** the supported process model can express a useful raw room/session program. Dependencies: REF-01. This feature supplies the operation/type inventory consumed by LANG-01 and EXEC-02; it does not approve arbitrary Rust types.

Build tasks:

- Propose typed protocol enums, owned records, identifiers, scalar operations, payload bytes and a membership collection.
- For each type/operation list construction, mutation, transfer, borrowing, clone/share behavior, suspension state, destruction and failure paths.
- Use accepted runtime-managed immutable buffers for shared payload bytes; exclude mutable shared application state.
- Specify exit-reason and error representations independently of application protocol payloads.

| Case | Branch and required treatment | Acceptance evidence / unresolved mapping |
|---|---|---|
| LANG-02.C01 | Scalar/enum/record state survives many scheduling turns. | Correct values and valid ownership after each resume; actual type support is demonstrated, not inferred from the old `u64` probe. |
| LANG-02.C02 | Owned data moves into spawn arguments, a send operation or the mailbox. | Exactly one accountable owner at every transfer boundary; sender exit cannot reclaim receiver-owned state. |
| LANG-02.C03 | A buffer is frozen/shared with several recipients. | Mutation is unavailable after sharing; each holder's reference has tracked lifetime; final release follows EXEC-03. |
| LANG-02.C04 | A nested value has custom `Drop`, interior mutability or an unqualified allocator/callback. | Reject until its full contract is qualified; accepting a container must not smuggle arbitrary element behavior. |
| LANG-02.C05 | Borrowed data would outlive its owner or cross a suspension/transfer incompatibly. | Compiler/checker rejects it or proves the representation valid; no pointer survives reclamation through an unchecked escape. |
| LANG-02.C06 | Collection growth, insertion/removal, lookup, copy or payload transform scales with input. | Define interruptible segments or a justified admitted bound, safe partial state and allocation-failure path. An ordinary short call site is not a cost bound. |
| LANG-02.C07 | A helper fails after partial initialization or mutation. | Every initialized element/resource remains accounted for; error behavior and cleanup are specified before admission. |
| LANG-02.C08 | Application data resembles `Down`/`Exit`, or code tries to construct a runtime envelope variant. | Resolve constructor visibility, receive matching and demonitor-flush semantics against REF-01.C03/C04; no implicit forgeable/unforgeable choice. |
| LANG-02.C09 | A reference error is catchable, while a direct kill is runtime control. | Rust representation preserves the distinction; exception/panic mapping remains a blocking design item for exposed operations. |

**Done when:** a source-level room/session sketch has no unexplained operation, every required type/operation has a contract, and representative cases compile through the public boundary. Exact container APIs remain subject to that evidence.

**Rust ownership/source refinements:** [report 41](../research/41-rust-guarantees-and-process-admission.md). These cases are planned qualification obligations. BEAM representation-specific branches are conditional reference work; a Rust alternative must meet the supported observable and ownership contract. Language-probe results do not qualify Bream features.

| Case | Branch, required treatment and acceptance evidence |
|---|---|
| LANG-02.C10 | Admitted code forgets an owning resource handle or retains it in ManuallyDrop; partial transfer/receiver death may overlap. Either enforced admission rejects the path or tracked cleanup obligations remain safely reclaimable without that destructor. Do not free a recipient-owned or escaped allocation to fix a leak. Rust probe acceptance is evidence of the language gap, not a Bream failure or passing Bream test. |

## LANG-03 — Generated suspension and resumable helpers

**Purpose:** supported CPU work relinquishes execution automatically while preserving valid state. Dependencies: LANG-01 and LANG-02. Evidence: [BEAM execution/BIF trace](../research/11-beam-scheduling-source.md); existing generated probe is only a starting mechanism.

| Case | Execution branch | Required evidence |
|---|---|---|
| LANG-03.C01 | Empty infinite loop; ordinary and nested loops; `continue`/`break` paths. | Every admitted cycle has mandatory runtime boundaries; another runnable process progresses. |
| LANG-03.C02 | Direct helper calls, recursion or mutually repeated helper work. | Support with covered cycles or reject explicitly; instrumenting only the caller is insufficient. |
| LANG-03.C03 | Long straight-line work, overloaded operators or hidden callbacks. | Establish maximum indivisible work and qualified calls; do not assume absence of a visible loop means inexpensive execution. |
| LANG-03.C04 | An operation suspends with partially copied/scanned/constructed state. | Resume produces the correct result; kill at every boundary safely releases all partial ownership. |
| LANG-03.C05 | Work allowance expires versus lifetime termination is requested. | Expiry preserves a runnable continuation; kill permanently stops it. A spent turn is not a process failure. |
| LANG-03.C06 | A guard or matcher performs input-sized work. | Generated/approved semantics participate in scheduling; no opaque predicate can bypass the contract. |
| LANG-03.C07 | Release optimization changes generated code. | Check behavior under the intended build flags; retain source/compiler/artifact identity. No reliance on debug-only checks. |
| LANG-03.C08 | Arbitrary native dependency never returns. | It fails admission or is outside the guaranteed process path; moving it to another worker does not count as successful process kill. |

Build resumable helpers with ordinary valid Rust ownership at supported cancellation boundaries. The execution representation/engine remains an experiment; this plan does not authorize arbitrary stack destruction or select the signal-fiber probe as a safe-kill implementation.

**Performance-source refinements:** [report 37](../research/37-checkpoint-and-continuation-costs.md). All remain planned. Reference-specific representations and optional/later capabilities are conditional; the Rust implementation must preserve the applicable ownership and observable behavior, not copy the storage layout.

| Case | Branch, required treatment and acceptance evidence |
|---|---|
| LANG-03.C09 | Inline checkpoint continues versus exhausted branch materializes a continuation. Same admitted computation and control obligations in both paths; test kill at the branch and forbid partially saved or post-death resumed state. |
| LANG-03.C10 | Helper transition omits selected saved fields versus full-state fallback. Closed helper classification; debug poison/negative controls detect forbidden reads; allocation, error and panic paths cannot use stale state. |
| LANG-03.C11 | First suspension expands the saved live set; later suspensions reuse capacity or shrink logical liveness. Track initialized values and resources independently of liveness; no lost ownership, stale reads or invalid partial cleanup after kill. |

**Rust ownership/source refinements:** [report 41](../research/41-rust-guarantees-and-process-admission.md). These cases are planned qualification obligations. BEAM representation-specific branches are conditional reference work; a Rust alternative must meet the supported observable and ownership contract. Language-probe results do not qualify Bream features.

| Case | Branch, required treatment and acceptance evidence |
|---|---|
| LANG-03.C12 | A generated process holds local borrows across a mandatory boundary in address-sensitive saved state, then resumes, transfers an eligible pinned owner or is killed after poll return. Preserve pinned storage, borrow validity, exclusive execution and qualified cleanup. The isolated Pending/drop probe establishes only its safe compiler-generated example; active destruction and arbitrary user Drop remain excluded. |

## REF-02 — Executable conformance and interleaving harness

Dependencies: REF-01 for expected behavior. Model tests can start before the full runtime; executable Bream comparisons attach as features appear.

| Case | Harness branch | Required result |
|---|---|---|
| REF-02.C01 | Deterministic ordered reference scenario. | Pin runtime identity, retain fixture and raw observations, compare against its specified contract. |
| REF-02.C02 | Race with several allowed observations. | Validate the allowed set and invariants; do not demand identical scheduling traces from two runtimes. |
| REF-02.C03 | Small transition model with explicit publication, sleep, death and ownership steps. | Enumerate bounded interleavings and counterexamples; state model bounds and abstraction limits. |
| REF-02.C04 | Native stress execution. | Assert safety/progress invariants with trace identifiers; stress success does not replace the model or a soundness argument. |
| REF-02.C05 | Runtime hangs or an external watchdog terminates the executable. | Record failure/containment, never a successful Bream process kill. |
| REF-02.C06 | Fault injection at allocation, publication or cleanup boundaries. | No orphaned ownership or forbidden post-stop work; distinguish recoverable failure from host-abort boundaries. |
| REF-02.C07 | Source/test inspected, but no matching executable run exists. | Evidence remains unexecuted. Never inherit an upstream test's pass status by reading it. |
| REF-02.C08 | A fixture exploits a weak upstream assertion or undocumented assumption. | Repair the oracle locally with documented reasoning; retain provenance and do not overstate what the original test proves. |

**Concurrency/clock refinements:** [report 49](../research/49-rust-atomic-protocol-observations.md). All cases remain planned. Source-specific coordinator/lock/provider details are conditional reference work, not a requirement to reproduce ERTS representation. Small model completion does not qualify Bream.

| Case | Branch, required treatment and qualification |
|---|---|
| REF-02.C09 | A candidate weak-memory optimization changes publication, final-owner visibility or parking synchronization. Retain positive and deliberately weakened models, exact ordering/bounds/tool identity and counterexamples; distinguish model completion from Rust soundness, hardware fairness and Bream preemption. All protocol-visible shared operations must be modeled or explicitly excluded. |

Use explicit handshakes/barriers where feasible; arbitrary sleeps are not proof of an interleaving. Retain per-case status and raw results separately from documentation link validation.

## APP-01 — Raw room/session integration

Dependencies: admitted state/helpers; process execution, send/receive/timeouts, monitors, exit control, cleanup, inspection and host shutdown. A small separately linked pair qualifies link behavior; the whole room is not implicitly one failure group. No GenServer, supervisor, networking or automatic restart.

| Case | Application branch | Required observation |
|---|---|---|
| APP-01.C01 | Join, duplicate join, leave, repeated leave and unknown member. | Write the fixture's application policy explicitly; these are room-protocol choices, not BEAM semantics. Use a membership set proposal, subject to the state sketch. |
| APP-01.C02 | Publish to zero, one and many members; query membership concurrently. | Correct typed messages, correlation and agreed application ordering; fanout work remains schedulable. |
| APP-01.C03 | Session dies before/during/after join or fanout. | Monitor-driven membership cleanup, no unintended incarnation confusion under LIFE-01.C07/C08's identity contract, no payload leak or invalid shared bytes. |
| APP-01.C04 | Room dies while sessions are waiting or sending. | Sessions can observe its death; sends follow dead-destination semantics. No implicit restart or migrated mailbox. |
| APP-01.C05 | Independent CPU hog; then a session itself becomes a hog. | Unrelated processes progress; host can kill it. Its own next application message is not magically handled during its computation. |
| APP-01.C06 | Parser/transform or membership scan handles large inputs. | Qualified helper path with partial-work cancellation. Payload parsing/fanout is a proposed representative workload, not a user-selected new algorithm requirement. |
| APP-01.C07 | Cleanup and host shutdown under active communication. | All owned references accounted for; no post-stop execution, late stale delivery or silent unfinished teardown. |

## MEASURE-01 — Density, fairness and cost report

Dependencies: correct APP-01, EXEC-06 observability and reproducible REF-02 evidence. This feature does not set unmeasured performance targets.

| Case | Measurement branch | Required reporting |
|---|---|---|
| MEASURE-01.C01 | Empty waiting processes versus realistic state at increasing counts. | Separate baseline, marginal RSS/PSS, virtual memory, logical ownership, native thread count and retained cleanup work. |
| MEASURE-01.C02 | CPU work, selective scans, fanout and cleanup compete. | Throughput, ready delay, longest indivisible segment, kill-to-stop and stop-to-reclamation distributions. |
| MEASURE-01.C03 | Same shared bytes referenced by many processes. | Physical storage and logical references reported separately; no multiplying one allocation into fictitious physical memory. |
| MEASURE-01.C04 | BEAM reference and async Rust baseline. | Comparable protocols/state, pinned versions, worker/hardware/build settings; do not claim an async baseline meets uncooperative CPU fairness. |
| MEASURE-01.C05 | Later multi-worker variation or a changed operation implementation. | Rerun affected measurements after correctness gates; no extrapolation from one worker or one arithmetic loop. |

The benchmark output is an operating-range report, not a hard latency, universal density or arbitrary-Rust compatibility promise.

**Performance-source refinements:** [report 37](../research/37-checkpoint-and-continuation-costs.md). All remain planned. Reference-specific representations and optional/later capabilities are conditional; the Rust implementation must preserve the applicable ownership and observable behavior, not copy the storage layout.

| Case | Branch, required treatment and acceptance evidence |
|---|---|
| MEASURE-01.C06 | Cold first-use allocation versus warmed state, and disabled versus enabled instrumentation. Separate cohorts, retain compiler/configuration/artifact identity and report observer overhead; do not merge modes into one claimed latency. |
| MEASURE-01.C07 | Offered rate exceeds service capacity, or the load generator itself stalls. Record offered/completed rates, enqueue delay, response time, horizon and retained backlog; preserve growing-mailbox semantics and expose coordinated omission. |
