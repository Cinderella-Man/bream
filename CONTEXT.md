# Bream process runtime

Language for investigating a reusable Rust runtime for small, independently scheduled processes. These terms describe the intended behavior; they do not select an execution engine.

## Working rule: research BEAM before asking

Before every question about how Bream should behave, first establish how BEAM/ERTS behaves from official documentation and, where needed, the pinned source code and tests. Use the existing source notes when they already answer the question. If the reference behavior is clear, replicate it and record the supporting evidence; do not ask the user to choose or reconfirm that behavior.

Ask only when research leaves a genuine ambiguity or a concrete conflict with Rust safety or an already accepted requirement. Explain the reference behavior and the exact unresolved conflict before asking. Do not turn implementation uncertainty into a request for the user to redesign BEAM semantics. This rule applies to every future design and grilling round.

## Working rule: investigate performance alongside behavior

For each process-runtime branch reviewed, consider allocation, copying, synchronization, cache locality, scheduling overhead and cleanup cost. Record plausible improvements as falsifiable experiments in the [performance backlog](docs/performance-plan.md), with baseline, workload, dependencies and acceptance evidence. Distinguish source facts, optimization hypotheses and measured results. Preserve automatic scheduling, safe forced termination and supported BEAM semantics; a throughput gain alone cannot justify worse unaccounted scheduling or cleanup behavior. Keep expanding this inventory when new evidence appears rather than claiming every possible speed improvement is known.

Always examine what Rust can eliminate or check earlier than the reference implementation: ownership transfers, typed protocols, invalid state transitions and unnecessary dynamic representation work. Record exactly what the compiler guarantees, what the public API must enforce and what still requires runtime coordination. Do not equate safe Rust, `Send`, pinning or destructor behavior with Bream admission, preemption or complete reclamation.

## Language

**BEAM process model**: The process-level execution, messaging and failure semantics that Bream takes as its reference, supplied by Erlang’s BEAM/ERTS runtime. _Avoid_: OTP behaviours as an interchangeable term.

**Process-runtime facility**: A mechanism that governs process execution, state ownership, addressing, communication, lifecycle, timing or observation. _Avoid_: every library bundled with Erlang/OTP; compression and cryptographic algorithms are application-library capabilities, not process-runtime facilities to port.

**BEAM conformance case**: A supported process-runtime scenario whose permitted and forbidden observations are derived from the BEAM reference; concurrent events may permit several outcomes. _Avoid_: one observed execution trace as a universal ordering guarantee.

**OTP behaviour**: A reusable application-level process pattern, such as a generic server or supervisor, built using runtime primitives. _Avoid_: process primitive or scheduler feature.

**Process**: One live native Rust computation with its own execution state and lifetime, which can be suspended and resumed independently of other processes. _Avoid_: OS process, actor, Wasm guest, or future as interchangeable names.

**Process admission**: Establishing that a program and its reachable operations and resources satisfy the process runtime's execution and lifetime contract. _Avoid_: ordinary Rust compilation, thread-transfer eligibility or a marker type alone as proof of admission.

**Scheduling preemption**: The runtime taking execution away from a running process so another runnable process can make progress, preserving the suspended computation for later resumption. _Avoid_: timeout, termination, or rollback.

**Explicit suspension**: An owned request that prevents a process from running application code until the applicable suspensions are released. _Avoid_: an ordinary scheduling turn ending; runtime control may still proceed.

**Suspension acknowledgment**: A runtime report of the target's suspension state at a defined observation point. _Avoid_: a permanent guarantee that the target remains suspended or proof that one particular requester still owns a suspension.

**Scheduling quantum**: A renewable allowance for one turn of process execution. Exhausting a turn makes the process yield; it does not make the process fail. _Avoid_: lifetime budget.

**Reduction**: A runtime-defined unit of accounted process work used to spend a scheduling allowance. _Avoid_: CPU instruction, elapsed nanosecond, or an identical unit across different runtimes.

**Execution quota**: An optional cumulative limit on a process's work, whose exhaustion can end that work according to an explicit policy. _Avoid_: scheduling quantum.

**Process incarnation**: One process lifetime, distinct from a replacement started after it exits. _Avoid_: stable service name when identifying one execution.

**Address publication**: The point at which a process incarnation becomes discoverable through the runtime's applicable identity lookup. _Avoid_: its first application instruction or the creator receiving its address; these observations can occur at different times.

**Resident state**: A process's state retained between interactions, without implying recovery after failure. _Avoid_: durable state.

**Owned message**: A message whose transferable contents become the receiving process's responsibility, without exposing mutable state shared between process lifetimes. _Avoid_: shared mutable handle as an equivalent transfer.

**Immutable buffer**: Runtime-managed byte storage that processes may reference without exposing mutable access; each holder owns its reference and the storage remains live while any owner needs it. _Avoid_: arbitrary shared Rust object or shared mutable buffer.

**Mailbox**: The process-owned collection of messages accepted for that process and not yet consumed by it; ordinary local send does not wait for consumption or a configured capacity slot. _Avoid_: a handler, delivery acknowledgment, or a queue shared by unrelated process lifetimes.

**Selective receive**: Consuming the earliest queued message that matches the current receive patterns while retaining unmatched messages in their relative order; the receive may have a timeout. _Avoid_: FIFO-only dequeue or discarding messages that do not match.

**Priority reception**: Eligible messages entering ahead of ordinary mailbox content while preserving the specified order within each class. _Avoid_: scheduler priority or reordered signal delivery.

**Process alias**: A process-owned alternative message address whose acceptance can be deactivated independently of the process lifetime. _Avoid_: a registered service name, delivery acknowledgment, or owner of execution.

**Message protocol**: The typed set of application messages a process accepts, expressed with Rust data types. _Avoid_: arbitrary Erlang terms or lifecycle identity as interchangeable concepts.

**Process signal**: Runtime-level communication concerning a process, including message delivery and lifecycle relationships. _Avoid_: Unix signal or application message as interchangeable names.

**Runtime notification**: A mailbox item exposing a runtime event to application receive, such as monitor-down or an eligible trapped exit. _Avoid_: every process signal; some signals act on the process without becoming mailbox items.

**Mailbox envelope**: The runtime-owned choice of an application protocol value or runtime notification presented to one selective receive. _Avoid_: separate application and lifecycle queues, or internal control signal as an interchangeable value.

**Monitor**: A one-way observation of a process lifetime, identified separately from the process, that reports its termination without automatically propagating that failure to the observer. _Avoid_: link or supervisor.

**Cancellation barrier**: The point after which a removed relationship can no longer cause new effects on the cancelling process, subject to the reference operation's rules about re-establishment. _Avoid_: peer cleanup completion, retraction of already accepted notifications, or termination of the peer.

**Death notification**: A monitor's report that a process has stopped executing and is no longer addressable as a live lifetime. _Avoid_: proof of completed memory reclamation or of rolled-back external effects.

**Reclamation completion**: Completion of the runtime's tracked release work for a dead process, including release of its owned references. _Avoid_: freeing storage still owned by another live process or an immediate reduction in host RSS.

**Cleanup obligation**: The runtime's responsibility to release a process-associated ownership or external registration when its defined lifetime ends. _Avoid_: assuming that forgetting an application handle, stopping execution or removing a process address discharges that responsibility.

**Readiness notification**: A report that an I/O operation may make progress; the operation must still establish its actual result. _Avoid_: completed I/O, reserved bytes or a guarantee that readiness persists until use.

**Registration lifetime**: The interval during which a backend association or work derived from it still carries access or release obligations. _Avoid_: the lifetime of one numeric descriptor, its application handle or its recipient process.

**Observation ownership**: Responsibility for keeping inspection or diagnostic state valid until it is consumed, discarded or retired. _Avoid_: keeping the observed process alive or requiring it to receive its observer's work.

**Record retention**: Keeping a process's identity and lifecycle metadata available for outstanding runtime work after its death. _Avoid_: keeping the process alive or granting access to its already-reclaimed application state.

**Reclamation domain**: The set of participants and access obligations whose completion determines whether a class of retired storage may be reclaimed. _Avoid_: assuming a scheduler stop covers every host thread or every retained resource.

**Quiescent interval**: An interval in which a participant makes no accesses protected by the relevant reclamation domain. _Avoid_: idle, inactive or blocked as sufficient evidence without the domain's access contract.

**Lock authority**: Permission to access particular shared state while its required protection is held. _Avoid_: process-record retention or permission to execute application code as interchangeable rights.

**Clock domain**: A time reference with its own origin and progression rules, against which instants and deadlines have meaning. _Avoid_: treating equal numeric timestamps from wall time and monotonic time as interchangeable instants.

**Link**: A bidirectional failure relationship between process lifetimes through which termination can deliver exit signals. _Avoid_: monitor, ownership hierarchy, or automatic restart policy.

**Pending unlink**: A locally inactive link whose removal protocol has not yet finished. _Avoid_: an active failure relationship or proof that all relationship storage has been reclaimed.

**Exit reason**: The reason associated with one process’s termination, used in death observation and failure propagation. _Avoid_: a restart decision or proof of complete external-effect rollback.

**Exit trapping**: A process policy that turns eligible exit signals into mailbox notifications; a direct forced-kill signal remains untrappable. _Avoid_: catching every failure or intercepting runtime cleanup.

**Forced termination**: Irreversibly stopping a process without requiring its application code to cooperate, while safely reclaiming the resources covered by the runtime's ownership contract. _Avoid_: stop request, indefinite suspension, or arbitrary user-destructor execution as synonyms.

**Runtime-owned resource**: A process-associated allocation or external handle whose lifetime and cleanup the runtime can track independently of application cleanup code. _Avoid_: any object created by a Rust closure, which may have untracked ownership and effects.
