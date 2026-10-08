# BEAM and OTP: the experience Bream should test

**Scope update:** the user subsequently selected BEAM/ERTS primitives first. GenServer, supervision and restart-framework recommendations below are comparative research, outside the initial Bream plan. [Accepted boundary](../adr/0004-beam-primitives-first.md), [current roadmap](../roadmap.md).

Research date: 2026-10-07. This is a proposal for experiments, not an accepted architecture or a promise of Erlang compatibility. It builds on the accepted native Rust, automatic scheduling, safe termination, owned-message, and ordinary Cargo requirements in [the interview](../design-tree.md).

Primary baseline: official documentation reported **Erlang/OTP 29.1.1** and **Elixir 1.20.4** when read. Source links below pin those release tags. These are reviewed sources, not local benchmark results; no BEAM comparison experiments were executed for this report.

## The important target

Recommendation: make an Elixir developer recognize **process identity, messages, failure relationships, supervised lifetime, and inspectable operation**. Syntax imitation alone would miss the point. The first useful demonstration should be a supervised stateful service remaining usable while an unrelated process consumes CPU, then recovering from a failed child without resurrecting its old identity or leaking its resources.

There are two different responsiveness questions. A scheduler can interrupt a process's computation so peers run; that does **not** make its own next mailbox message execute halfway through its current callback. OTP's server implementation calls the user callback and interprets its result before returning to its receive loop. An indefinitely computing callback can therefore leave that server's own requests unanswered while other processes continue. This follows from the [versioned `gen_server` implementation](https://github.com/erlang/otp/blob/OTP-29.1.1/lib/stdlib/src/gen_server.erl). Bream should demonstrate both behaviors explicitly rather than suggesting preemption implies reentrant handlers.

## Semantics to preserve or deliberately distinguish

### Identity, startup, links, and monitors

An Erlang PID identifies a process, not a durable service. The current language reference guarantees PID uniqueness among live processes on connected nodes and explicitly permits eventual reuse after termination. It does not promise an eternally unique identifier. [OTP data types](https://www.erlang.org/doc/system/data_types.html#pid)

`spawn_link` creates the child and link atomically. `spawn_monitor` creates the child with a monitor rather than requiring a racy later registration. The source marks monitor creation as atomic and implements it through `spawn_opt(..., [monitor])`. [OTP 29.1.1 `erlang.erl`](https://github.com/erlang/otp/blob/OTP-29.1.1/erts/preloaded/src/erlang.erl#L10626-L10669)

Monitors report death in a `DOWN` message carrying a monitor reference, target, and reason; monitoring an already dead process still reports death. A monitor notification is observation, not automatic propagation of failure to the observer. Elixir exposes name lookup/registration separately from PID identity. [Elixir `Process`](https://elixir.hexdocs.pm/1.20.4/Process.html#monitor/1)

OTP adds a separate startup handshake: `proc_lib:start_link` waits for initialization acknowledgement while linking atomically; merely spawning a process does not establish that its service is ready. [OTP `proc_lib`](https://www.erlang.org/doc/apps/stdlib/proc_lib.html#start_link/5)

**Proposed Bream interpretation:** use an incarnation-specific process handle, a distinct service-name lookup, and a distinct monitor token. Give local stale handles stronger protection through generation validation; define exhaustion rather than silently wrapping. Test immediate child failure before the parent runs again. Do not conflate “spawned”, “initialized”, and “available under a name”. Atomic monitored spawn is a foundational operation, while an actor startup handshake can sit above it.

### Send, receive, and mailbox work

Ordinary Erlang `receive` selects the earliest queued matching message, tests that message's clauses in source order, removes only the selected message, and leaves unmatched messages queued. Scanning can cost linear time in the number of preceding messages; even `after 0` may scan the whole queue. Sending to a dead PID does not report application delivery failure. [OTP expressions](https://www.erlang.org/doc/system/expressions.html#receive)

Signal order is guaranteed per sending entity and destination, not globally across senders. OTP 28 introduced opt-in priority reception, which can change mailbox placement without violating signal arrival order. Thus “all messages are FIFO” is too broad even for BEAM. [OTP process signal semantics](https://www.erlang.org/doc/system/ref_man_processes.html)

Message data is generally copied between BEAM processes; same-node reference-counted binaries and literals are exceptions. The receive implementation can skip older messages for a newly created reference when the compiler proves the relevant pattern, making correlated calls cheaper than arbitrary mailbox scans. [OTP process efficiency guide](https://www.erlang.org/doc/system/eff_guide_processes.html#sending-messages)

**Proposed Bream interpretation:** a typed owned enum can provide familiar matching while moving ownership instead of copying immutable terms. That is a useful Rust distinction, not identical BEAM semantics. The matcher must borrow and inspect before consuming the chosen message. An arbitrary Rust predicate could loop, mutate state, panic, or hide unsupported calls; the checked process model must qualify match guards and scanning itself. Budget mailbox scanning as runtime work, not only user loops.

Overload policy remains a product choice. A bounded mailbox with rejection, backpressure, or dropping has different sender behavior from familiar `send`. Specify whether failure returns the owned payload and exactly when ownership leaves the sender. “Accepted into a mailbox” must not imply “handled”, and a retry after an ambiguous response must not imply exactly-once effects.

### Calls, casts, timeouts, and reply capabilities

OTP `gen_server:call` waits for a reply or exits the caller on error/timeout. Since OTP 24 it uses aliases to suppress late replies. A cast is asynchronous and does not acknowledge that application handling occurred. [OTP `gen_server`](https://www.erlang.org/doc/apps/stdlib/gen_server.html#call/3)

An alias is a revocable destination. Deactivation drops subsequent alias-directed messages before they enter the mailbox; messages already queued are not retroactively removed. [Elixir process aliases](https://elixir.hexdocs.pm/1.20.4/Process.html#module-aliases)

There is a documentation discrepancy worth retaining in the test plan: Elixir 1.20.4's `GenServer.call` prose still warns about late replies, but its implementation delegates to `:gen.call`. OTP 29.1.1's finite-timeout path creates a monitor alias, revokes it on timeout, and checks for a reply already received during the race. Use the pinned implementation as the behavioral reference, not the outdated prose. [Elixir source](https://github.com/elixir-lang/elixir/blob/v1.20.4/lib/elixir/lib/gen_server.ex#L1171-L1187), [OTP `gen.erl`](https://github.com/erlang/otp/blob/OTP-29.1.1/lib/stdlib/src/gen.erl#L251-L280)

**Proposed Bream interpretation:** test a typed single-use reply capability that can be revoked on timeout or caller death. It need not be literally an Erlang alias. The server may already have performed its effect when the caller times out; response abandonment is not request cancellation or rollback. Returning a Rust `Result` versus failing the caller is a deliberate API decision still to interview. A transparent automatic retry would be particularly dangerous for non-idempotent requests.

### Exit, shutdown, and cleanup

For ordinary active links, non-trapping receivers generally ignore `normal` and fail on abnormal exits. Trapping converts eligible exits into messages. A direct `kill` signal bypasses trapping and produces `killed`; the word `kill` propagated through a link is not that same unconditional mechanism. [OTP exit-signal rules](https://www.erlang.org/doc/system/ref_man_processes.html#receiving-exit-signals)

`GenServer.terminate/2` is not guaranteed to run. A supervisor can request shutdown, wait the child's configured timeout, then forcibly kill it; brutal kill skips that callback. Correct resource ownership cannot depend on a best-effort application cleanup callback. [Elixir `GenServer.terminate/2`](https://elixir.hexdocs.pm/1.20.4/GenServer.html#c:terminate/2)

BEAM's `DOWN`/linked-exit sequencing waits for *directly visible* language resources, such as registered names, but excludes heap-held resources, dirty native execution, and final PID reclamation. Dirty native code can continue after logical process exit. [OTP resource-release qualification](https://www.erlang.org/doc/system/ref_man_processes.html#directly-visible-erlang-resources)

Native extensions also do not inherit normal BEAM scheduling automatically: long NIFs must split work or use appropriate native scheduling facilities, and resource destructor timing is not guaranteed. [OTP NIF manual](https://www.erlang.org/doc/apps/erts/erl_nif.html#long-running-nifs)

**Proposed Bream interpretation:** distinguish a stop request, irreversible execution stop, exit observation, and completed cleanup of the resources Bream actually owns. This need not force four public events; it forces an unambiguous specification. If `join` promises every tracked I/O operation is reconciled, that may be a stronger promise than BEAM `DOWN`. Keep graceful callback shutdown separate from mandatory kill. A process author's busy loop must not suppress kill by catching a panic or ignoring a mailbox message.

### Supervision means lifecycle ordering

OTP child policy distinguishes permanent (restart after termination), transient (restart only after abnormal termination), and temporary (never restart) children. Exceeding restart intensity in its interval terminates the supervisor and its children. Erlang's raw supervisor defaults are one restart per five seconds. [OTP supervisor](https://www.erlang.org/doc/apps/stdlib/supervisor.html)

Elixir defaults instead to three restarts per five seconds. Its familiar strategies are one-for-one, one-for-all, and rest-for-one. Static children start in specification order and stop in reverse order, so dependency order has operational meaning. [Elixir Supervisor](https://elixir.hexdocs.pm/1.20.4/Supervisor.html)

Supervision shuts children down with a configurable grace interval and escalation to kill. Child supervisors need appropriate shutdown treatment: interrupting a subtree supervisor while it is stopping descendants can leave lifecycle races. [OTP supervision principles](https://www.erlang.org/doc/system/sup_princ.html)

**Proposed Bream interpretation:** begin with one-for-one in an optional layer, but test initialization failure, failed restart, graceful timeout, and parent death immediately. A supervisor requires a reusable recipe for constructing fresh process state. A moved `FnOnce` capture cannot by itself provide arbitrary repeated restarts. The recipe must also respect the admitted code and cleanup model. Restart restores availability through a new incarnation; recovering old application state requires a separate state source.

### Observation is part of the developer experience

Observer presents process identifiers, names/labels, reductions, memory, and queue length; it can expose detailed state and trace process/message/scheduling events. [OTP Observer](https://www.erlang.org/doc/apps/observer/observer_ug.html#processes-tab)

OTP's `sys` offers status/state extraction, tracing, suspension, and code-change protocols. These depend on processes implementing the system-message protocol, directly or through behaviors, and requests can time out. They are not universal arbitrary-memory inspection. [OTP `sys`](https://www.erlang.org/doc/apps/stdlib/sys.html)

**Proposed Bream interpretation:** expose runtime metadata independently of user callbacks: incarnation, label, execution state, mailbox count/bytes, scheduling work, monitored relationships, and exit reason. Keep application state snapshots opt-in, bounded, and redacted. A stuck handler must not prevent inspecting its queue or killing it. A GUI, arbitrary state mutation, remote shell, and hot code upgrades can wait.

## Falsifiable experiments in dependency order

These experiments are proposed, not run. Use a small executable reference fixture on a pinned OTP release where semantics are meant to match; record deliberate Rust differences separately. Synchronize with barriers/events, not lucky sleeps. Record complete histories for race tests and compare them with a stated sequential model.

| Experiment | Setup and decisive observation | Failure that blocks the next layer |
|---|---|---|
| **E1: useful owned state** | Port a counter/key-value process with a typed enum, approved owned strings/buffers, helper computation, and timer. Keep one unrelated loop permanently runnable. No handwritten scheduling checkpoints. | The useful program requires an untracked escape, manual yield, or an ordinary dependency call that can monopolize a worker. |
| **E2: ownership transfer against death** | Inject sender and receiver termination before admission, during transfer, after enqueue, during selection, and after dequeue. Track payload allocation/ownership/drop. | Leak, double drop, freed payload reachable by receiver, or an ambiguous contract about who owns a rejected send. |
| **E3: spawn/monitor/death** | Child exits on its first instruction; register monitoring atomically; repeatedly reuse internal slots; race monitor cancellation with death. Test already-dead targets and multiple independent monitor tokens. | Lost death observation, duplicate outcome for one token, old PID resolving to replacement, or cancelled monitors corrupting unrelated messages. |
| **E4: mailbox semantics under load** | Two senders provide ordered sequences, mixing unmatched messages and correlated replies. Scale stale-mailbox length while peers compute and answer pings. | Per-sender reorder in the chosen ordinary mode, lost unmatched messages, matcher escape, or an unsliced scan starving peers. |
| **E5: timeout/reply/death race** | Delay a state-changing request beyond timeout; race reply, caller death, server death, and revocation. Repeat with a reply already queued. | Late-response pollution, stale reply hitting a later call, resource loss, duplicate accepted reply, or an API implying that timeout reversed the effect. |
| **E6: exit policy matrix** | Normal, abnormal, requested shutdown, and forced kill across linked/unlinked, trapping/non-trapping processes; kill during compute, receive, and tracked I/O. | Kill suppressible by application code; reason confusion; process resumes after irreversible stop; cleanup completes while owned work still touches freed state. |
| **E7: one supervised service** | Service fails during initialization and after readiness; restart fails repeatedly; grace expires; parent dies; service owns a name and resource. | Restart begins with unsafe old ownership, stale handles hit replacement, descendants escape shutdown, or a restart storm avoids escalation. |
| **E8: operational responsiveness** | Observe queue, scheduling progress, exit history, and tracked bytes under full worker saturation. Ask the hog's own handler for a reply separately. | Runtime inspection/kill starves with application work, or the demonstration mistakes peer fairness for reentrant service responsiveness. |

Use progress and ordering assertions before choosing latency targets. Then measure p50/p99/max peer response latency, kill-to-stop latency, stop-to-cleanup latency, throughput, mailbox scan work, and bytes per idle/active process on declared hardware. A run's measured maximum is not a hard deadline. Do not quote the existing generated future's small representation size as total process cost.

## Scope recommendation for the interview

**Preserve the semantic core:** process-local lifetime and state; automatic scheduling; owned messages; failure observation; atomic lifecycle relationships; safe kill; explicit cleanup completion; useful runtime inspection.

**Prototype the familiar service layer early:** typed call/cast, initialization acknowledgement, one-for-one supervision, and local names. These are recommendations for evaluating the developer experience, not a requirement to place actors in the scheduler crate.

**Make explicit choices:** selective receive versus FIFO-only protocols; mailbox overload behavior; whether failed calls return `Result` or fail the caller; exact link/trap-exit vocabulary; meaning of `DOWN` versus cleanup completion; restart recipe; admission of helpers and container operations.

**Defer breadth:** distribution, transparent location naming, live code upgrade, ETS-like shared tables, process dictionaries, priority reception, a remote shell, and the full suite of OTP behaviors. Preserve extension points only where the first concrete experiments require them. Neither distributed durability nor hostile native-code isolation follows from a BEAM-like process API.
