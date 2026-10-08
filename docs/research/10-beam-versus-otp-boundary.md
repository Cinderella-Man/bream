# BEAM primitives first: the boundary for Bream

Research date: 2026-10-07. Primary references are Erlang/OTP 29.1.1 and Elixir 1.20.4. Source links pin release tags. This report responds to the user's explicit scope correction: **familiar process primitives in Rust first; no GenServer, supervisor, or other OTP behavior required for the first experiments.** Classification is factual; the proposed experiment order is a recommendation.

## Four meanings that should stay distinct

**BEAM** is the abstract machine that executes Erlang object code; this is why compiled modules use the `.beam` suffix. Bream is borrowing process semantics, not planning a BEAM bytecode interpreter or Elixir execution environment. [OTP compilation documentation](https://www.erlang.org/doc/system/code_loading.html)

**ERTS** is the runtime implementation layer relevant to these semantics: process scheduling, process state, signal handling, and runtime services. In the tagged source, scheduler/process machinery lives in [`erts/emulator/beam/erl_process.c`](https://github.com/erlang/otp/blob/OTP-29.1.1/erts/emulator/beam/erl_process.c), signal machinery in [`erl_proc_sig_queue.c`](https://github.com/erlang/otp/blob/OTP-29.1.1/erts/emulator/beam/erl_proc_sig_queue.c), and the `erlang` API declarations/wrappers under [`erts/preloaded`](https://github.com/erlang/otp/blob/OTP-29.1.1/erts/preloaded/src/erlang.erl). Calling these “BEAM primitives” is useful product shorthand; not every exposed function is literally a VM opcode or implemented entirely in C.

**OTP behaviors and libraries** build reusable organization above raw processes. The official design principles distinguish behaviors such as generic servers and supervisors, and application/release organization. Bream does not need these abstractions to expose processes and messages. [OTP design principles](https://www.erlang.org/doc/system/design_principles.html)

**Erlang/OTP**, as the distribution/project name, is broader than those behaviors: its own README includes the runtime, reusable components, and design principles under OTP. Consequently, “from the OTP repository” does not mean “a high-level OTP behavior”. [OTP 29.1.1 README](https://github.com/erlang/otp/blob/OTP-29.1.1/README.md)

The practical Bream boundary is therefore **a selected local process/runtime surface**, not “everything named BEAM/ERTS” and not “everything except files shipped by OTP”.

## Classification of the requested surface

The references identify where a capability belongs. The final column is proposed prioritization, not a claim that the user has approved every API or all ERTS functionality for version zero.

| Capability | Reference layer and examples | Bream experiment priority |
|---|---|---|
| Spawn and process identity | Runtime process creation: `spawn`, `self`, PID; atomic `spawn_link`/`spawn_monitor` forms. [ERTS source](https://github.com/erlang/otp/blob/OTP-29.1.1/erts/preloaded/src/erlang.erl#L10626-L10669) | Foundation: qualified native program, incarnation handle, automatic scheduling, normal completion. |
| Mailbox, send, selective receive | Language/runtime mechanism: `!`, `send`, and `receive`; selective receive leaves unmatched messages queued. [Expressions](https://www.erlang.org/doc/system/expressions.html#receive) | Foundation: owned payload transfer, explicit send outcome, typed matching, fair scan/wakeup. |
| Exit, trap-exit, forced kill | Runtime failure signaling and process flags; does not require a behavior callback. [Process semantics](https://www.erlang.org/doc/system/ref_man_processes.html#receiving-exit-signals) | Foundation: qualify safe irreversible stop first; test the exit/trapping policy matrix separately. |
| Link and unlink | Runtime relationship operations, including atomic linked spawn. [ERTS source](https://github.com/erlang/otp/blob/OTP-29.1.1/erts/preloaded/src/erlang.erl#L6852-L6888) | Early lifecycle experiments; application failure coupling without restart policy. |
| Monitor and demonitor | Runtime observation, monitor references, and `DOWN`; independent of supervisors. [ERTS source](https://github.com/erlang/otp/blob/OTP-29.1.1/erts/preloaded/src/erlang.erl#L2030-L2072) | Early lifecycle experiments: immediate death, observation races, cancellation, stale handles. |
| Receive timeout | Language/runtime `receive ... after`. [Expressions](https://www.erlang.org/doc/system/expressions.html#receive) | Initial room/session fixture; match-versus-timeout race and generated suspension. |
| Message timers | ERTS `send_after`, `start_timer`, `cancel_timer`; distinct from the higher-level `timer` module in STDLIB. [ERTS source](https://github.com/erlang/otp/blob/OTP-29.1.1/erts/preloaded/src/erlang.erl#L6009-L6038), [STDLIB timer](https://www.erlang.org/doc/apps/stdlib/timer.html) | Small local timer surface when the fixture needs it; no full timer-module clone. |
| References and aliases | Runtime-generated reference values and revocable process destinations; not GenServer-specific. [ERTS source](https://github.com/erlang/otp/blob/OTP-29.1.1/erts/preloaded/src/erlang.erl#L2203-L2212), [process aliases](https://www.erlang.org/doc/system/ref_man_processes.html#process-aliases) | Correlation token early; alias/reply-destination revocation as a subsequent primitive experiment. |
| Basic process inspection | ERTS `process_info`; no GenServer necessary. [ERTS source](https://github.com/erlang/otp/blob/OTP-29.1.1/erts/preloaded/src/erlang.erl#L5573-L5580) | Small runtime-owned status snapshot for every experiment; no arbitrary state reflection required. |
| Basic local name registration | Runtime `register`, `unregister`, `whereis`; names are removed on process termination. [Registered processes](https://www.erlang.org/doc/system/ref_man_processes.html#registered-processes), [runtime implementation](https://github.com/erlang/otp/blob/OTP-29.1.1/erts/emulator/beam/register.c) | Correctly classified as primitive, but optional for first fixture: pass handles directly. |

PID versus service-name identity is an important distinction even without automatic restart. A manually spawned replacement must not become the process addressed by an old live Rust handle. The generation/identity policy is Bream's own engineering contract, not an assumption that Erlang PIDs are globally unique forever; Erlang explicitly permits eventual reuse. [OTP data types](https://www.erlang.org/doc/system/data_types.html#pid)

## What is above the boundary

| Facility | Actual layer | Treatment now |
|---|---|---|
| `proc_lib` | STDLIB process-starting conventions, initialization acknowledgement, diagnostic ancestry. [Documentation](https://www.erlang.org/doc/apps/stdlib/proc_lib.html) | Not needed to spawn a raw process. A fixture can send its own `Ready` message. |
| `gen_server`, `gen_statem`, `gen_event` | STDLIB behavior modules and callback protocols. [OTP design principles](https://www.erlang.org/doc/system/design_principles.html) | Defer; no behavior trait, `handle_call`, or generic server wrapper in the primitive qualification. |
| `supervisor` | STDLIB restart/lifecycle policy over linked processes. [Documentation](https://www.erlang.org/doc/apps/stdlib/supervisor.html) | Defer; links and monitors remain independently useful. |
| `application` | Kernel library application lifecycle/packaging interface. [Documentation](https://www.erlang.org/doc/apps/kernel/application.html) | Defer; a Rust host can explicitly construct and shut down the test runtime. |
| Elixir `Registry` | Elixir library with key-based process lookup, unique/duplicate entries, and optional partitioning. [Documentation](https://elixir.hexdocs.pm/1.20.4/Registry.html) | Defer; distinct from ERTS's basic local registered-name facility. |
| Observer and `sys` | Tooling and library protocols over lower-level runtime facilities. [Observer](https://www.erlang.org/doc/apps/observer/observer_ug.html), [`sys`](https://www.erlang.org/doc/apps/stdlib/sys.html) | Use minimal Bream metadata rather than recreating these tools/protocols now. |

Distribution, hot code loading, ETS, ports, priorities, and other runtime facilities may also be implemented below the behavior layer. That location alone is **not** an argument to include them in the first local Bream implementation. The selection criterion is whether a primitive is necessary to prove the accepted native scheduling, isolation/ownership, termination, and useful-process requirements.

## Two distinctions the raw-process contract must retain

**Runtime signal reception is not application receive.** BEAM handles signals independently of evaluating a `receive` expression. A message signal may enqueue a message; an exit signal may instead transition the target to exiting. A process stuck in computation does not thereby execute its next message handler. [OTP signal reception](https://www.erlang.org/doc/system/ref_man_processes.html#receiving-signals)

For Bream, this means mandatory generated checkpoints can be where runtime control regains execution without pretending that the application called `receive`. A kill request must not simply sit behind user messages awaiting the process author's cooperation. A trapped exit, by contrast, is application-visible information under the chosen policy. Scheduling peers, accepting messages, and consuming messages are separate observables.

**Logical death is not proof of all native cleanup.** BEAM delays `DOWN` and linked exits until directly visible language resources are released, but excludes heap-held resources, dirty native execution, and final PID reclamation from that specific guarantee. [OTP resource boundary](https://www.erlang.org/doc/system/ref_man_processes.html#directly-visible-erlang-resources)

Bream must name what its death observation guarantees and independently qualify reclamation of its approved Rust state and outstanding native operations. This is a primitives problem, before any supervisor exists. It does not authorize resuming stopped code, leaking indefinitely, skipping required Rust cleanup, or claiming cancellation reverses external effects.

## Minimal local experiment sequence

1. **Raw process plus owned state.** Expand the generated native subset just enough for a checked enum protocol, admitted owned data, bounded helper work, and persistent local state. One worker runs this process alongside a pure CPU loop. Both make progress without handwritten yields. Kill the loop and account for every tracked allocation. Reject escape paths at compile time.
2. **Mailbox and receive.** Add owned send/receive, then typed selection and timeout. Race enqueue, wakeup, receive selection, timeout, sender death, and receiver death. Every payload must have one accountable owner; unmatched messages stay available; mailbox scanning also relinquishes execution through runtime scheduling.
3. **Raw lifecycle relationships.** Add monitor/demonitor and atomic monitored spawn, then link/unlink and trapping semantics. Test child death before parent resumption, cancelled monitor delivery, repeated independent monitors, and death during owned transfer. No restart loop is needed.
4. **Room/session fixture.** Run one raw room process and several raw session processes. Sessions send `Join`, `Leave`, and owned chat messages. The room maintains membership and fans out admitted payloads. It monitors sessions and removes dead incarnations. A reference-correlated query or `Ready` handshake uses ordinary messages, not a `GenServer.call` API. A timeout marks an inactive session according to fixture policy. An unrelated hog tests peer progress; a hog inside a session proves that its own inbox need not be consumed. Kill a session and verify other sessions and the room survive, membership converges, and tracked state is reclaimed. Keep room handles explicit; networking, registration, and automatic restart are unnecessary.
5. **Small adjacent primitive probes.** Add revocable reply destinations, timer cancellation races, and local name removal/re-registration only when each answers a concrete next question. Inspect runtime status, mailbox size, scheduling work, and termination/cleanup throughout; a dashboard is not a prerequisite.

The fixture's membership cleanup is application logic built from monitoring. It is not supervision: it observes a vanished participant and does not restart it. If the room dies, the test can report the room's death and stop. Success is a credible process runtime with familiar raw-process concepts; recovery frameworks can be evaluated later on top of that foundation.
