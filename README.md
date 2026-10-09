# Bream

**Small processes. Fair turns.**

**Machine constraint, 2026-10-08:** the user runs other work on this machine. No speed tests, benchmarks or performance comparisons may run here. Continue with correctness validation only. All earlier speed-test evidence has been removed and must be redone elsewhere. [Persistent instructions](AGENTS.md).

Research toward a standalone Rust library for **preemptively scheduled, safely terminable native Rust processes**. CPU-heavy code must be suspended and resumed automatically, without application-written yields or awaits. Initial scope: Linux x86-64, ordinary Cargo integration, runtime-aware I/O/synchronization, process-owned state and owned messages. First workload: long-lived stateful services.

**Investigation baseline: 2026-10-07; latest implementation experiment: 2026-10-09. This repository contains research, native mechanism probes, a restricted useful-process runtime experiment and pinned BEAM fixtures. It is not a production runtime or published crate.**

**Current milestone: persistent-read timeouts passed on the injected-clock single-worker profile.** A checked process can time out a read, retain earlier bytes and reuse the same endpoint after request settlement. A claimed successful read may win serviced expiry; old timer callbacks cannot affect later requests. Six configurations cover seven stop boundaries, 96 data/timeout and 96 kill/timeout races. All 517 expected command outcomes pass, with 153 runtime tests per compiler profile and six socket reference cases on each of two BEAM builds. [Milestone status](docs/milestones.md), [implementation and limits](experiments/timed-read/README.md). The clock is injected; hard deadlines, writes, transfer and full G1–G5 qualification remain open. Next is bounded runtime-owned writing for a local echo exchange. No speed measurements were made.

## What we found

**Native scheduling and restricted safe stop now have executable evidence; the full library remains unproven.** Signals demonstrate native preemption. Generated code demonstrates automatic scheduling and safe cleanup, now extended into useful buffer processing, private service records and checked helpers through a public boundary. Broader resource and lifecycle qualification remain open. See [the findings](docs/findings.md) and [forced-termination analysis](docs/research/05-forced-termination.md).

The accepted research direction is a **Cargo-generated native process model**: mandatory checkpoints, checked operations/types, owned state/messages, and runtime-controlled cleanup. The second [mechanism probe](experiments/generated-process/README.md) stops an infinite process after a generated checkpoint, reclaims its tracked 4 KiB buffer, and lets its peer finish; all ten unsupported-code fixtures are rejected. It is not a wrapper accepting unrestricted native Rust functions. [Model and tradeoffs](docs/generated-process-model.md).

**The core native behavior is possible, and we reproduced it.** Using pinned `open-coroutine-core` 0.7.0, two native Rust computations each performed 100 million iterations on one task-worker OS thread. Both progressed before either finished, with 25 observed task-to-task changes and no yields/awaits in their loops. Disabling preemption made the same source run serially. Both builds produced identical results. A helper thread supplies scheduling interrupts; it does not execute the computations. [Probe, reproduction and limitations](experiments/native-preemption/README.md).

**There is no native preemptive scheduler to carve out of the old lab.** Its demonstrated preemption comes from Wasmtime; Tokio and Kameo provide the surrounding execution/actor services. The valuable extraction for this project is its behavior requirements, conformance scenarios, and separation between resident application state and runtime services. Bream's native engine requires new work or adoption/hardening of a native upstream implementation. [Source audit](docs/research/01-lab-extraction-audit.md).

**A standalone library boundary makes sense.** Own native process execution, continuation lifetime, preemption, parking/wakeup, completion, and scheduling metadata. Keep actors, supervisors, distribution, persistence, and hot upgrades above it. A regular Rust program should be able to use it for two CPU jobs without adopting an actor framework. [Proposed design](docs/native-design.md).

**Native scheduling leaves compatibility and termination obligations open.** Interrupting a loop is established. Interruption around allocation, thread-local state, locks, signals, unwinding and native dependencies needs a defined contract and further evidence. Native processes also do not automatically inherit BEAM's tiny heaps or failure isolation. [Native mechanisms](docs/research/02-native-mechanisms.md), [candidate audit](docs/research/03-runtime-landscape.md).

**Termination claim checked, 2026-10-09:** thirty bounded Rust/BEAM observations separate cancellation, cleanup and peer integrity. Safe Rust permits a child thread to outlive its parent future and a destructor to block the next task on a worker. Controls also confirm that BEAM does not roll back shared ETS updates or automatically stop unlinked children. These are contract obligations, not evidence that preemption is fully solved or Rust needs a tiny crate allowlist. [Findings and limits](docs/research/59-termination-boundary-observations.md), [experiment](experiments/termination-boundary/README.md).

**Rust's green-runtime history checked, 2026-10-09:** the original Rust 0.12 runtime executes successfully in bounded correctness probes. Source and removal PRs show cooperative scheduling and an earlier, separately removed linked-cancellation system. Modern controls explore operation-owned buffers and consuming registration APIs as useful Bream directions. [Research and conclusions](docs/research/63-rust-green-runtime-lessons-and-experiments.md), [reproducible probes](experiments/rust-green-history/README.md).

## Planning the next experiments

The accepted first scope is **BEAM/ERTS process primitives in Rust**: processes, mailboxes, send/receive, monitoring, linking and lifecycle control. Messages use typed Rust protocols, growing mailboxes and selective receive with timeout; links follow BEAM-style exit/trapping rules. The reference workload is a room/session example built directly from those primitives. GenServer and supervisor frameworks are outside this initial scope. [Accepted scope](docs/adr/0004-beam-primitives-first.md), [revised experiment roadmap](docs/roadmap.md), [BEAM versus OTP boundary](docs/research/10-beam-versus-otp-boundary.md).

The user also selected one runtime mailbox envelope for typed payloads and lifecycle notifications, runtime-managed immutable byte sharing, and `Down` after execution stop with tracked reclamation separately observable. Observable primitive behavior should follow BEAM by default, including ordinary sends silently discarding messages addressed to dead processes.

The local [BEAM source map](docs/beam-source-map.md) now traces the actual scheduler, interpreter/x86 JIT, mailboxes, signals, spawn/relationships, exit cleanup, memory, native work, timers and I/O at a pinned OTP commit. The original traces and edge-case audits are extended by [deeper process reports and reference observations](docs/process-runtime-inventory.md), including a complete inventory of the 21 declared control-signal kinds in live/exiting dispatch. Each distinguishes traced branches from partial coverage and unproven properties. Bundled ZIP/compression/crypto algorithms are outside the port. The [latest reference pass](docs/research/29-pinned-reference-observations.md) builds pinned OTP29.1.1 and tests targeted cases on release JIT and debug interpreter VMs, including a reproduced marker assertion with controls. Broader feature experiments remain planned; the [milestone tracker](docs/milestones.md) identifies completed restricted implementations. Open Rust-specific decisions are recorded in the [design tree](docs/design-tree.md).

**Behavior coverage is incomplete.** The [coverage audit](docs/behavior-coverage.md) separates known BEAM contracts, allowed race outcomes, unresolved Rust mappings and unexecuted conformance tests. Its edge-case inventories supplement the source walkthrough; they are not a claim that every case has been specified or tested.

The ownership depth pass traces [buffered send versus death](docs/research/30-buffered-signal-publication-protocol.md), [receive parking and wakeup](docs/research/31-receive-parking-wakeup-protocol.md), and [unlink/demonitor cancellation](docs/research/32-relationship-cancellation-protocol.md). Two [finite interleaving models](docs/research/33-finite-ownership-interleavings.md) exercise lifetime handoff and direct-message parking; five deliberately broken variants produce concrete counterexamples. These are bounded protocol models, separate from the executed BEAM fixtures and Rust mechanism probes.

The [comprehensive build plan](docs/build-plan.md) organizes this work into **40 feature groups and 537 numbered branch cases**, with implementation tasks, dependencies, allowed race outcomes, forbidden results and qualification gates. Detailed plans cover admission/conformance, execution/resources, messaging, lifecycle and [deeper process-runtime branches](docs/build-plan/process-runtime-expansion.md). The catalogue distinguishes initial refinements from later features and representation-specific reference research. Every feature remains planned; unresolved mappings are visible work items rather than assumed behavior.

The [performance investigation](docs/performance-plan.md) contains **107 planned experiments** under the responsiveness-first priority. All speed-test evidence and derived performance claims have been removed at the user’s request; every affected test must be redone in a suitable authorized environment. Source analysis, structural improvements and correctness work continue. No speed tests may run on this machine. [Removal record and audit](evidence/performance-reset/README.md).

## Start here

1. [Milestone status](docs/milestones.md): what has actually passed, current work and the next bounded exit condition. The [build inventory](docs/build-plan.md) retains the 537 cases and broader qualification gates.
2. [Findings and recommendation](docs/findings.md): extraction verdict, BEAM lessons, viable implementation routes, and what remains unproven.
3. [Native design](docs/native-design.md): generic library boundary, proposed state model, API example, and compatibility issues.
4. [Executed native probe](experiments/native-preemption/README.md): source, pinned dependencies, negative control, logs, hashes, and replay commands.
5. [Generated-process probe](experiments/generated-process/README.md): checked native process bodies, automatic checkpoints, and ordinary cleanup after cancellation.
6. [Qualification plan](docs/qualification-plan.md): mechanism qualification background, supplemented by the current build-plan gates.
7. [Design tree and interview record](docs/design-tree.md): user decisions separated from proposals.

The glossary is [CONTEXT.md](CONTEXT.md). Accepted decisions live in the [ADR directory](docs/adr/), with the latest [BEAM compatibility default](docs/adr/0010-beam-observable-semantics-by-default.md) guiding the source-informed interview. [Naming notes](docs/naming.md) explain the selected working name and record registry checks; no name was reserved or published.

## Evidence map

| Item | Evidence status |
|---|---|
| Native same-worker preemption | Freshly executed; preemption-enabled/disabled builds compared. |
| Correct results for the probe's arithmetic | Identical checksums across both builds; complete iteration counts. |
| General native safety | Unproven; the mechanism probe deliberately excludes allocation, locks, I/O and TLS access inside its loops. |
| Generated scheduling and kill/cleanup | Freshly executed for the restricted macro grammar and tracked buffer; not a general native-kill proof. |
| Full safe process API and owned-message/resource model | Required, unproven; a restricted mailbox implementation now runs, with broader resource and protocol qualification still open. |
| Tiny native process density | Unmeasured. Wasm density from the old lab does not establish it. |
| Source-lab results | Retained evidence reviewed and final input hashes checked; two existing compiled unit tests rerun. Full lab qualifications were not rerun. |
| BEAM, Go and native candidate claims | Primary-source review, with version/source links in reports. |
| Local BEAM/ERTS implementation review | Pinned source, protocol traces and 21-kind control-dispatch inventory. Exact-version core build and targeted fixtures now executed; full suite and Bream conformance remain unexecuted. |
| Finite ownership/wakeup models | Two small sequentially consistent baseline graphs explored; five broken variants detected. No weak-memory or complete-runtime proof. |
| Local naming and Rust variance controls | Eight reference families in three pinned VM profiles; four Rust compilation attempts and two executions. Targeted observations, not exhaustive registry or admission qualification. |
| Rust ownership and worker-notification costs | Four Loom controls retained; all speed measurements removed and requiring a new run. |
| Restricted useful-process public boundary | Executed through a separate consumer; 22 negative fixtures per profile, shadowing controls, useful buffer work, external kill and cleanup correctness controls. |
| Structured state and helper extension | Executed through a separate consumer; 55 negative fixtures per profile, shared allowance across helpers, partial-construction cleanup, native frame layouts and ownership controls; speed measurements removed. |
| Stable identities and terminal lifecycle | Executed: stale IDs, runtime namespace checks, stop/reuse/cleanup ordering, accepted-kill arbitration, publication rejection and shutdown. Performance samples removed; new performance qualification required. |
| Owned mailboxes and parking | Correctness-only: 155 expected command outcomes, 21 unit tests per profile, six public controls, 96 send/kill races and 132 expected compile rejections. Fixed-tag/any receive, exact ownership and logical scan/cleanup sharing; timeouts are covered by the following milestone. |
| Receive timeouts | Correctness-only with injected time: 183 expected command outcomes, 35 unit tests per profile, 96 send/timeout and 96 kill/timeout races, and 150 expected compile rejections. Old callback isolation, bounded service and cancellation cleanup; real-time driver remains open. |
| Local monitors and Down | Correctness-only: 221 expected command outcomes, 51 unit tests per profile, six monitor controls, 96 creation/kill and 96 cancellation/kill races, 178 compile rejections and ten positive shadowing executions; inherited mailbox/timeout controls also pass. |
| Local links and exit trapping | Correctness-only: 263 expected command outcomes, 68 unit tests per profile, six link controls, 96 link/kill and 96 unlink/kill races, 210 compile rejections and twelve positive shadowing executions. Inherited controls pass; logical worker checks cover fatal exit without another poll and shared cleanup accounting. |
| Atomic monitored/linked spawn | Correctness-only: 311 expected command outcomes, 83 unit tests per profile, six spawn controls, 96 spawn/parent-kill and 96 spawn/shutdown races, 248 compile rejections and fourteen positive shadowing executions. Initial topology precedes the first child poll; rejection rolls back metadata and owned input. |
| Bounded room/session integration | Correctness-only: 317 expected command outcomes; six application configurations cover eighteen conversation/death/shutdown scenarios. Retained expected transcript and observed collector checkpoints validate routing, failure isolation and timeout order; all tracked input/message receipts settle. Runtime/compiler semantics and 83 unit tests per profile are inherited. |
| Complete Bream public runtime | Not implemented; experimental signatures are not a finalized production API. |

Source laboratory: `/home/bat/projects/rust-actor-runtime-lab`, commit `f78658b08faaea84b7c9e7bb3a2dab13fafc17c7`. Its source was left unchanged. The local project is on Git branch `codex/research`, with no remote or published package.

The user selected **Bream** as the working name on 2026-10-07: BEAM with an R for Rust. Source, crates, macros, commands, and project paths use Bream. [Historical evidence](evidence/archive/pre-bream/README.md) preserves the original logs and tested artifacts; current results were regenerated after the rename.
