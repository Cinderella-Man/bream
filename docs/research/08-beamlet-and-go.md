# Beamlet and Go: familiar processes, different guarantees

**Scope update:** the user subsequently selected BEAM/ERTS primitives first. GenServer, supervision and restart-framework recommendations below are comparative research, outside the initial Bream plan. [Accepted boundary](../adr/0004-beam-primitives-first.md), [current roadmap](../roadmap.md).

Research date: 2026-10-07. This is a source and documentation audit for Bream's next experiments, not a recommendation to switch Bream to Wasm. Bream's accepted native Rust, ordinary Cargo, automatic scheduling, owned-message and forced-termination requirements remain the baseline. No comparator tests were executed in this review.

The useful distinction is between **an OTP-shaped API**, **fair scheduling**, and **enforced process death with resource reclamation**. A project can provide the first two while leaving an infinite application callback alive. Bream should qualify all three separately.

## Sources and reproducibility

| Project | Inspected revision | Evidence scope |
|---|---|---|
| Beamlet | `b60dd1102298d8d0b8d061b15c7708f2d72af7f3`, main, 2026-08-30 | Public SourceHut repository cloned successfully, despite the web reader failing to open it. README describes v0.4.2; the v0.4.2 tag resolves to the earlier `ac4f81874504f0ae7230410191b0fcc7c18ba110`. Audit uses the main commit explicitly. |
| Beamlet dependencies | Go directive `1.25.10`; wazero `v1.12.0` | Pinned [go.mod](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/go.mod). |
| Ergo | `301097b967006d6c2329de23f0cc352fd340a3c7`, master, 2026-09-07 | Public GitHub source cloned; [version.go](https://github.com/ergo-services/ergo/blob/301097b967006d6c2329de23f0cc352fd340a3c7/version.go) reports framework release 3.3.0. This is a commit audit, not an assertion that all web documentation corresponds to one release tag. |
| Go | Runtime source at `go1.27.1`, plus official standard-library contract | Relevant machinery and cancellation semantics, not a Bream-compatible reusable runtime. |

Beamlet's [author introduction](https://developmeh.com/projects/beamlet/) describes an OTP-inspired substrate underneath an agentic tool harness. It is written in Go, but its application processes execute Wasm instances. Its public repository is [git.sr.ht/~ninjapanzer/beamlet](https://git.sr.ht/~ninjapanzer/beamlet). The substrate and the agentic harness should be evaluated separately.

## Beamlet: what is actually there

### Small substrate and lifecycle

The public surface comprises `NewSupervisor`, `StartChild(Spec)`, `Send`, `Inbox`, `Terminate`, `Upgrade`, and `Events`. `Spec` packages a name, Wasm bytes, capabilities, restart policy and mailbox size. Default mailbox capacity is 64. Supervision is one-for-one, with permanent/transient/temporary policies and a restart window. The default is three restarts per minute. Exhaustion abandons that child; it is not a demonstrated full OTP supervision tree escalation mechanism. `Events` has a bounded buffer and drops events when consumers lag. These are source-verified behaviors, not just the article's mapping. [Supervisor source](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/otp/supervisor.go).

Each incarnation receives a new numeric PID, but routing uses names. A message holds a sender name and opaque byte payload. Process state includes separate graceful-stop, cancellation and completion channels. The completion channel closes after the execution call returns and deferred runtime close runs. [Process definitions](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/otp/process.go), [supervisor source](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/otp/supervisor.go).

### Scheduling is not its kill mechanism

Termination requests graceful stop, waits the configured grace period, cancels the context if necessary, then waits for completion. The runtime is configured with `WithCloseOnContextDone(true)`. This enables wazero's inserted periodic termination checks. It is not a power that `context.CancelFunc` has over arbitrary native Go code. [Supervisor source](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/otp/supervisor.go), [wazero configuration contract](https://pkg.go.dev/github.com/tetratelabs/wazero@v1.12.0#RuntimeConfig).

Its upstream spinning-guest test sends a spin command, uses a 300 ms grace period, and fails if `Terminate` has not returned within ten seconds. Other tests cover restart identity, restart exhaustion, filesystem grants, mailbox-full reporting and invalid-module upgrades. These tests were read, not rerun. The spin test establishes the project's intended cancellation behavior; it does not measure resumption fairness, fairness under saturation, or a ten-second production latency guarantee. [Upstream tests](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/otp/supervisor_test.go).

Bream should borrow the shape of a runaway-process test but add the missing distinction: first prove that a hog repeatedly loses and regains CPU without dying; then separately kill it and verify reclamation.

### Messages and call semantics

At the guest boundary, send copies bytes out of guest memory. Receive removes the next message and copies only its payload into the guest buffer; an undersized buffer loses that message with an error. There is no selective-receive operation in this ABI. The host-side `Send` accepts a Go byte slice without copying, so its ownership discipline differs from the guest boundary. [Host ABI](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/otp/host.go), [send implementation](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/otp/supervisor.go).

The delivery ADR deliberately leaves crash-restart mailboxes behind. Upgrade instead attempts to forward the old mailbox. Its rationale recognizes poison-message restart loops and rejects building durable delivery into the substrate. Treat this as Beamlet's own policy; its statements equating bounded mailbox-full errors or call-layer retries with BEAM semantics should not substitute for an OTP contract audit. [Delivery ADR](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/docs/adr/0004-best-effort-message-delivery.md).

The harness call helper permits one pending call per tool name, retries sends while the name is missing, then waits for a reply or timeout. Correlation is name-based rather than a substrate request identifier. A timeout does not prove the work was undone. This is insufficient as Bream's general call contract, especially across restarts and late replies. [Harness `callTool` and routing](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/harness/loop.go).

The README explicitly leaves guest-to-guest links/monitors and a general call/reply ABI for future work. It also distinguishes killable loops from CPU metering. Thus Beamlet is a useful small-system comparator, not evidence that the complete Elixir process experience fits in its existing API. [README omissions](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/README.md).

### Resources and deployment: inspect the edges

Capabilities enumerate filesystem mounts, allowed HTTP/TCP endpoints, TLS roots, clocks, randomness, memory pages and shutdown grace. A per-process native connection table translates guest integer handles to host connections. This separation is a useful Bream ownership pattern, without adopting a hostile-code sandbox promise. [Capability definitions](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/otp/caps.go), [TCP handle table](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/otp/tcp.go).

There is a source-level cleanup concern: `defer table.closeAll()` belongs to `instantiateHostModule`, which returns before later guest TCP operations create connections. No later close-all hook is visible in the inspected path. Therefore this review does **not** treat all native connection reclamation on process death as established. TCP reads/writes use deadlines, which is another reason to test kill during an external operation, not only during arithmetic. These are audit findings requiring execution to quantify; no leak measurement was performed. [Host registration and I/O functions](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/otp/host.go), [deadline calculation](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/otp/tcp.go).

The upgrade ADR favors registering a replacement before draining the old instance and explicitly accepts a period with both alive. That is service replacement, not automatic transfer of resident state or arbitrary BEAM code loading. In the inspected implementation, compilation precedes registration but guest execution starts asynchronously; readiness has no explicit handshake. Consequently, its bad-binary test should not be generalized to every failure during initialization. Bream should use a separate readiness contract if replacement is ever added. [Upgrade ADR](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/docs/adr/0005-hot-swap-registers-new-before-draining-old.md), [upstream upgrade tests](https://git.sr.ht/~ninjapanzer/beamlet/tree/b60dd1102298d8d0b8d061b15c7708f2d72af7f3/item/otp/supervisor_test.go).

## Go itself: scheduling without arbitrary forced death

Go has asynchronous goroutine preemption; its runtime uses safe-point metadata and checks whether an interrupted execution point permits preemption. This is compiler/runtime integration, not merely a general signal handler applicable to opaque Rust frames. It is useful as a specification and adversarial workload baseline. [Go 1.14 runtime announcement](https://go.dev/doc/go1.14#runtime), [Go 1.27.1 preemption implementation](https://github.com/golang/go/blob/go1.27.1/src/runtime/preempt.go).

Cancellation is a separate protocol. A context exposes a done signal; a `CancelFunc` does not wait for the application work to stop. Code must return in response or call an operation that does. The Go team's pipeline guidance explicitly explains that abandoned goroutines retain resources and must exit themselves. A runnable goroutine can be scheduled fairly while never accepting cancellation. [Context contract](https://pkg.go.dev/context#CancelFunc), [Go pipeline cancellation guidance](https://go.dev/blog/pipelines).

This is a particularly valuable negative control for Bream: another process progressing does not imply the runaway process can be destroyed safely.

## Ergo: a much closer OTP user experience, with a different kill contract

Ergo supplies actor callbacks, calls and responses, names, links, monitors and supervisors over native Go. Source-verified actor handlers dispatch one message at a time; `HandleMessage` and `HandleCall` sit above the lower-level process behavior. Calls carry a `Ref` and can use a timeout; replies use `SendResponse`. [Actor implementation](https://github.com/ergo-services/ergo/blob/301097b967006d6c2329de23f0cc352fd340a3c7/act/actor.go), [call/link/monitor implementation](https://github.com/ergo-services/ergo/blob/301097b967006d6c2329de23f0cc352fd340a3c7/node/process.go).

Its supervisor API includes one-for-one, all-for-one, rest-for-one and dynamic same-type children, with permanent/transient/temporary policies and restart-intensity controls. This is useful API vocabulary and behavior to compare against actual Elixir programs. It should not make every strategy a Bream first-experiment requirement. [Supervisor definitions](https://github.com/ergo-services/ergo/blob/301097b967006d6c2329de23f0cc352fd340a3c7/act/supervisor.go).

**A running Ergo process is not forcibly interrupted by `Kill`.** The inspected `node.Kill` sets `ProcessStateZombee` and returns for running, initializing or waiting-response states, retaining registration until the goroutine stops. The run loop finishes termination after application execution returns; the actor loop checks the state between callbacks. An infinite native callback can therefore remain executing after kill was requested. This is a direct inference from the code paths, not a reproduced runtime test. It does not satisfy Bream's accepted forced-termination contract. [Kill implementation](https://github.com/ergo-services/ergo/blob/301097b967006d6c2329de23f0cc352fd340a3c7/node/node.go#L1949), [run-loop cleanup](https://github.com/ergo-services/ergo/blob/301097b967006d6c2329de23f0cc352fd340a3c7/node/process_run.go), [callback dispatch](https://github.com/ergo-services/ergo/blob/301097b967006d6c2329de23f0cc352fd340a3c7/act/actor.go#L133).

Memory isolation is also different. Local routing places the supplied Go value directly into the mailbox. Maps, slices and pointers can retain shared mutable memory. Ergo's documentation candidly identifies this as programmer discipline and mentions its `argus` vet check; remote encoding creates a copy. Bream's accepted owned-message model should be enforced by its admitted data model, rather than inferred from sequential callbacks. [Local delivery source](https://github.com/ergo-services/ergo/blob/301097b967006d6c2329de23f0cc352fd340a3c7/node/core.go#L14), [documented isolation boundary](https://docs.ergo.services/basics/actor-model).

For observability, the source exposes process identity, registered name, parent, execution state/time, mailbox counts, message totals, running time, wakeups, links and monitors. This data model is worth considering early, even with no graphical observer. A killed-but-still-executing state being visible is better than falsely reporting cleanup complete. Bream can make its stronger lifecycle measurable through similarly explicit introspection. [ProcessInfo](https://github.com/ergo-services/ergo/blob/301097b967006d6c2329de23f0cc352fd340a3c7/gen/process.go#L1093).

## Experiments these comparisons suggest

These are proposed falsifiable experiments, not accepted product decisions. They extend Bream's generated-process probe rather than restarting the engine investigation around Wasm.

| Experiment | What to construct | Pass/fail evidence |
|---|---|---|
| **Useful stateful process** | A typed counter/session service with enum messages, owned state, timer, a CPU-heavy helper and call/reply. Use the same behavior specification in a small Elixir reference. | No authored yield/cancellation polling; generated helper loops participate in scheduling. Reviewers can explain lifecycle and failures using familiar process concepts. Unsupported helper/type constructs are rejected with actionable diagnostics. |
| **Death has completion** | A hot loop with an owned buffer, pending timer and process-owned socket; kill at every operation phase. Include a Go/Ergo infinite-callback negative control if that comparison is executed. | Distinguish request acceptance, execution stopped and reclamation complete. After the last state, no progress counter, timer delivery or resource registration remains; peer continues. Record distributions instead of inventing a hard deadline. |
| **Mailbox ownership under death** | Transfer an owned payload while killing sender/receiver before admission, after admission, and during dequeue. Repeat with a full mailbox. | Exactly one owner or one completed reclamation; no alias, leak, double free or acceptance after incarnation death. An error returns ownership or documents runtime disposal. Name reuse must not deliver to an unintended incarnation. |
| **Late replies and restart** | Caller times out; responder replies late; service restarts under the same name; caller issues another request. | Old reply cannot satisfy the new request. Timeout does not claim work rollback. Crash neither silently replays non-idempotent work nor transfers the old mailbox without a chosen policy. |
| **Supervision with a hog** | One-for-one supervisor with a hot child and an unrelated sibling; crash/kill/restart the child repeatedly. | Sibling stays responsive. New incarnation gets new identity. Restart decision follows confirmed death. Intensity exhaustion has an explicit outcome. Cleanup records balance across many generations. |
| **Inspect while saturated** | Capture PID/name/incarnation, state, mailbox length/bytes, turns, current admitted operation, owned resources and termination stage while all workers are busy. | Inspection and lifecycle control still progress. It reports meaningful distinctions between blocked I/O, CPU work, queued work and cleanup; instrumentation cost is measured. |

Choose a mailbox overflow policy explicitly before judging familiarity. An Elixir-like API cannot quietly imply both a bounded queue that rejects sends and unchanged BEAM send behavior. The same applies to selective receive versus typed callback dispatch: use a real request/reply example to discover whether the difference affects the intended workflow.

The most valuable lesson for Bream is not a larger feature checklist. It is a concrete developer journey: start a stateful process, address it, call it, observe it, kill a runaway, understand its exit, and watch a supervisor replace it without unrelated services stalling. Making that small journey truthful would distinguish Bream more than importing every OTP name.
