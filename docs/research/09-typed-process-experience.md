# Typed process experience: lessons for Bream's next experiment

**Scope update:** the user subsequently selected BEAM/ERTS primitives first. GenServer, supervision and restart-framework recommendations below are comparative research, outside the initial Bream plan. [Accepted boundary](../adr/0004-beam-primitives-first.md), [current roadmap](../roadmap.md).

Research snapshot: 2026-10-07. This bounded comparison uses official documentation and published source for Gleam OTP / Erlang libraries 1.3.0, Ractor 0.16.5, Kameo 0.22.2, and the current official Pony tutorial. Ractor and Kameo release metadata were checked against crates.io; their releases are dated 2026-08-07 and 2026-07-18 respectively. Pony tutorial pages are unversioned and should be read as a dated documentation snapshot. No external framework was executed in this investigation.

The requirements remain those recorded in [the design tree](../design-tree.md): native execution, automatic resumable CPU scheduling, safe forced termination, owned messages/state, runtime-managed cleanup, and investigation of an ordinary-Cargo generated model. The proposals below are experiments, not additional user decisions or a selected public API. Execution-engine comparisons are in [report 03](03-runtime-landscape.md); the present focus is the experience above that engine.

## What should feel familiar to an Elixir developer?

A useful target is a process that owns resident state, receives a typed protocol, makes synchronous-looking calls without blocking its scheduler worker, and has a distinct identity and observable lifetime. A supervisor can start a replacement after failure. None of that should require the developer to split CPU work into manually scheduled handler fragments.

**Recommendation to investigate:** borrow Gleam's distinction between a process identity and a typed message address, Pony's insistence that transferable data cannot retain mutable aliases, and Rust actors' request/reply conveniences. Keep Bream's stronger scheduling and termination requirements explicit. An actor wrapper should be one way to program a Bream process; sequential handlers alone are not the core runtime.

## Bounded comparison

| System | Useful API lesson | Important boundary |
|---|---|---|
| Gleam on BEAM | `Pid` for lifecycle; `Subject<Message>` for typed communication; selectors combine subjects and lifecycle notifications. | Its OTP actor wrapper and raw process receive functions have different unmatched-message behavior. Its kill semantics come from BEAM. |
| Pony | Typed asynchronous behaviours and compiler-enforced reference capabilities distinguish ownership transfer from immutable sharing. | The scheduler is explicitly non-preemptive. A behaviour that does not return monopolizes a worker. |
| Ractor | One associated message type, separate actor state, lifecycle events, parent-child supervision and optional monitors. | Signal priority is implemented around async work; a non-returning handler poll prevents that work from being interrupted. |
| Kameo | Per-message `Message<M>` implementations, typed replies, message-specific recipients, links and restart policies. | `kill` aborts a future; it cannot interrupt a poll that never returns. Supervised restarts can preserve queued messages. |

The details and sources supporting these distinctions follow.

### Gleam: typed addresses without losing process operations

Gleam's `Subject(message)` belongs to its creating process. Other processes can send values of that type; the owner receives them. `Selector(payload)` combines subjects, mapping different incoming types into one result. `Pid` remains available for lifecycle operations. The process module also exposes monitors, trapped exits, and untrappable kill. These are related interfaces, not one oversized actor handle. [Gleam process API 1.3.0](https://gleam-erlang.hexdocs.pm/1.3.0/gleam/erlang/process.html).

Typed selection is not a promise of arbitrary Erlang pattern-based selective receive. The documented way to choose different message orders is separate subjects, or Erlang receive code. Further, the OTP actor wrapper discards and warns about received messages excluded by its selector. Copying that wrapper's behavior into a Bream mailbox that promises to retain unmatched messages would be a semantic error. [Process receiving and sending](https://gleam-erlang.hexdocs.pm/1.3.0/gleam/erlang/process.html#send), [actor selecting](https://gleam-otp.hexdocs.pm/1.3.0/gleam/otp/actor.html#selecting).

The actor API has an initialization handshake, returns the started pid plus application data, and handles messages sequentially through a state-to-next-state callback. Its synchronous call helper crashes the caller on timeout. These are ergonomic choices to compare, not defaults Bream has accepted. Sequential handling within one actor does not mean a CPU-heavy callback prevents other BEAM processes from being scheduled. [Actor API](https://gleam-otp.hexdocs.pm/1.3.0/gleam/otp/actor.html).

Gleam's supervisors keep restart policy separate from child code: static trees support one-for-one, one-for-all and rest-for-one, with restart-intensity limits. A worker shutdown can escalate from a shutdown request to kill after its allowance expires. Bream should not advertise that escalation until its own generated programs and runtime resources satisfy the stronger kill contract. [Static supervisor](https://gleam-otp.hexdocs.pm/1.3.0/gleam/otp/static_supervisor.html), [child specification and shutdown](https://gleam-otp.hexdocs.pm/1.3.0/gleam/otp/supervision.html).

### Pony: transfer is a type-system property; preemption is separate

Pony behaviours are typed asynchronous invocations, and each actor executes one behaviour at a time. This is a useful alternative to a single protocol enum, but it is not a process-local selective-receive API. [Pony actors](https://tutorial.ponylang.org/types/actors.html).

Its `iso` capability represents isolated data that can be handed to another actor, `val` permits immutable sharing, and `tag` supports identity without ordinary data access. Only `iso`, `val` and `tag` meet the sendability restrictions. `consume` removes the sender's usable reference when transferring isolated ownership. The lesson is to define which aliases survive a send, not simply to annotate an envelope as a message. [Reference capabilities](https://tutorial.ponylang.io/reference-capabilities/reference-capabilities.html), [capability matrix](https://tutorial.ponylang.io/reference-capabilities/capability-matrix), [consume](https://tutorial.ponylang.io/reference-capabilities/consume-and-destructive-read.html).

Pony's scheduling guide explicitly says a behaviour must return for another actor to use that worker; it warns about infinite loops and long-running FFI. Therefore capability safety and low actor overhead do not answer Bream's CPU-hog requirement. This bounded review did not establish BEAM-equivalent links, monitors, supervision or an untrappable per-actor kill protocol in Pony; none should be inferred from its ownership guarantees. [Pony scheduling](https://tutorial.ponylang.org/gotchas/scheduling.html).

### Ractor: lifecycle channels are useful, but inspect the interrupt mechanism

Ractor's actor trait separates state from the actor implementation and uses an associated message type. Its documented local message bound is `Send + 'static`. Supervision links establish parent-child relationships, while monitors are an optional feature. The actor listens to kill signals, stop requests, supervision events and user messages with distinct priorities. A graceful stop lets current handler work finish; it is distinct from a kill. [Ractor 0.16.5](https://docs.rs/ractor/0.16.5/ractor/).

The implementation of `ActorPortSet::run_with_signal` races a handler future against the signal receiver using async `select!`; `kill` sends that signal. This can cancel suspended async work. It does not regain control from a handler stuck inside one poll. Tokio explicitly documents that blocking one select branch blocks the others on that task. The preemption limitation is an inference from the implementation and executor contract, not a claim that Ractor lacks useful cancellation. [Ractor signal implementation, lines 104–145 and 419 onward](https://docs.rs/ractor/0.16.5/src/ractor/actor/actor_cell.rs.html), [Tokio select runtime behavior](https://docs.rs/tokio/latest/tokio/macro.select.html#runtime-characteristics).

Ractor supplies a useful vocabulary for events and supervision; those types must not become a substitute for Bream's independent process execution and cleanup model.

### Kameo: ergonomic typed requests, with meaningful restart differences

Kameo permits separate message structs with `Message<M>` handlers and typed replies. `ActorRef<A>` can be narrowed to a message-specific `Recipient<M>` or `ReplyRecipient`. It distinguishes graceful mailbox draining, kill requests, and waiting for completed shutdown. Its reference-counted lifetime also means dropping all strong actor references can stop an actor. That is a policy to assess explicitly for long-lived services. [Kameo ActorRef 0.22.2](https://docs.rs/kameo/0.22.2/kameo/actor/struct.ActorRef.html).

Current Kameo includes supervision strategies and restart factories, not merely sibling links. Its lifecycle API documents that pending messages can survive a supervisor restart; the undelivered-message hook runs for terminal stops instead. A message already being handled when killed is not recoverable through that hook. [Actor lifecycle and undelivered messages](https://docs.rs/kameo/0.22.2/kameo/actor/trait.Actor.html), [supervision implementation and factories](https://docs.rs/kameo/0.22.2/src/kameo/actor.rs.html).

The kill implementation calls `futures::future::AbortHandle::abort`; the actor loop is wrapped in `Abortable`, after which teardown and hooks run. The futures API explicitly says an in-progress poll continues until it returns. Thus Kameo's immediate-kill terminology does not establish Bream's no-yield CPU-loop termination, and its asynchronous cleanup hooks do not establish bounded cleanup. [Kill source, lines 306–308](https://docs.rs/kameo/0.22.2/src/kameo/actor/actor_ref.rs.html), [actor loop, lines 240–257](https://docs.rs/kameo/0.22.2/src/kameo/actor/spawn.rs.html), [AbortHandle contract](https://docs.rs/futures/latest/futures/future/struct.AbortHandle.html#method.abort).

## Two semantic differences that must stay visible

**Typed is not process-owned.** Rust's `Send + 'static` can admit `Arc<Mutex<T>>`; ordinary thread safety permits shared mutable state. The standard library explicitly supports that pattern. A typed mailbox does not enforce Bream's accepted ownership boundary or controlled cleanup. Transfer eligibility and process-state cleanup eligibility need separate, enforceable rules. [Rust Arc thread-safety contract](https://doc.rust-lang.org/std/sync/struct.Arc.html#thread-safety).

**Restart is not necessarily a new address with a fresh queue.** An OTP supervisor starts a replacement process; that new incarnation has a new pid and does not inherit the dead process's mailbox. It can re-register a stable name. In contrast, Kameo's documented supervised restart preserves pending mailbox work. Neither policy gives exactly-once handling of the interrupted request. [Erlang process creation, identities and registration](https://www.erlang.org/doc/system/ref_man_processes.html), [OTP supervisor child startup](https://www.erlang.org/doc/apps/stdlib/supervisor.html), [Kameo restart-aware undelivered hook](https://docs.rs/kameo/0.22.2/kameo/actor/trait.Actor.html#method.on_undelivered).

For Bream, compare an incarnation-specific `Pid`/typed address against a separately named service endpoint. Keeping queued work across restart would require an explicitly longer-lived mailbox owner, resource accounting and replay policy. It should not happen accidentally because a Rust handle remains cloneable.

## Proposed semantic experiments

These are falsifiable next steps for [the generated model](../generated-process-model.md), not an implementation commitment. Names such as `Address<M>` and `Reply<T>` below are explanatory placeholders.

### 1. An owned typed protocol for a small stateful service

Express a counter or small key/value service using an owned protocol enum, approved resident structs, `match`, and a typed reply capability. Compare this with a per-message handler facade over the same process. Include a generated CPU helper that runs for many quanta: both forms must preserve peer progress without source-level yield/await calls.

Qualification: a wrong message or reply type fails compilation; a transferred runtime buffer cannot be read by the sender; termination frees buffers still owned by the sender, its mailbox or its current computation exactly once. Kill immediately before enqueue, after enqueue and after dequeue to verify ownership handoff. Borrowed payloads, shared mutable graphs, arbitrary type aliases and user-defined cleanup remain outside this experiment until specifically qualified. A decorative derive over unrestricted Rust fields is not enforcement.

### 2. Typed selection with a defined unmatched-message policy

Compare a simple FIFO protocol mailbox with a typed selector over protocol variants, reply IDs, timers and monitor events. Write down whether unmatched messages remain queued. For a retaining selector, test that skipped entries preserve relative order and are released if the owner dies. Include a backlog of nonmatches and a late matching reply.

Scanning and matching are executable work. Every long scan needs a renewable scheduling budget; matching must use approved bounded patterns or transformed guards. An arbitrary `Fn(&Message) -> bool` can loop, call foreign code or run overloaded operations and is excluded. Kill during a scan must free each retained payload once. A user-loop checkpoint alone does not cover this runtime path.

### 3. Synchronous-looking requests without worker blocking

A generated call should park the process while peers continue. Test one reply, no reply, reply after timeout, receiver death, and requester death. A one-shot typed reply capability can make the expected response clear while keeping request correlation out of routine application code.

Compare explicit `Result` on timeout with a process-exit convenience matching Gleam's call style; do not silently choose either. Establish that a timeout does not prove the request was unprocessed and does not undo external side effects. Reclaim or reject late reply payloads without retaining a dead caller's mailbox indefinitely.

### 4. Lifecycle operations that mean different things

Give the experiment distinct operations for graceful stop, forced kill, observing an exit, and waiting for reclamation. Graceful draining versus stopping after the current message is another explicit comparison. Run an infinite generated helper while a monitor and peer service remain active. Kill must make the victim permanently non-runnable independently of its ordinary mailbox selection, and its reported cleanup completion must match the resources actually released.

Prototype one-way monitors separately from failure-propagating links. Test monitoring before startup, during termination, and after exit; test concurrent unlink/death and duplicate notifications. Do not hide kill behind a user handler or let an arbitrary cleanup callback become necessary for runtime reclamation. Hard time bounds and full OTP signal ordering remain separate qualification work.

### 5. Restart identity and pending requests

Use a minimal supervisor with one child factory and a restart limit. Kill the child in CPU work, then start a new incarnation with fresh resident state. Record old/new identity, what a stale address observes, where each queued payload is owned, and what callers learn about pending requests.

Compare a fresh-mailbox replacement against an explicitly service-owned mailbox that survives restarts. The latter needs tests for obsolete requests, duplicate side effects, protocol-version compatibility and resource accounting during the gap. A stable service name should resolve to the new incarnation without making a monitor of the old incarnation silently observe the new one.

### 6. A small supervision policy above the process core

Add one-for-one behavior as a separate consumer of spawn, monitor, exit and kill primitives. Test repeated failure, exhausted restart allowance, parent termination during child restart, and failed initialization. Then evaluate whether one-for-all/rest-for-one and linked-service APIs belong in a separate layer.

The experiment should distinguish restartable resident state from durable state. It should not imply that cloning constructor arguments restores pre-failure service state or that supervision repairs external effects.

## Exact scope limits for this proposed slice

Start with local typed processes, a closed set of approved data shapes/runtime resources, generated helpers and mandatory checkpoints covering user and runtime work. The proposed slice excludes unrestricted Rust callbacks/traits/operators, arbitrary futures, borrowed cross-process data, shared mutable application state, arbitrary user `Drop`, unqualified blocking dependencies, and predicates outside the checked boundary. This is a suggested experiment boundary, not a claim that the user selected every exclusion for a final product.

Distribution, hot code upgrades, durable mailbox replay, transparent service-address retargeting, hostile-code security and guaranteed wall-clock deadlines are not established by these API comparisons. Pony's capability system does not certify a Rust macro; BEAM's kill behavior does not transfer with Gleam's type signatures; and Rust actor cancellation does not automatically cover CPU hogs. Bream's next evidence should be a useful stateful protocol surviving those adversarial lifecycle schedules within its admitted language, rather than a larger catalogue of actor method names.
