# Bream

**Small processes. Fair turns.**

Research toward a standalone Rust library for **preemptively scheduled, safely terminable native Rust processes**. CPU-heavy code must be suspended and resumed automatically, without application-written yields or awaits. Initial scope: Linux x86-64, ordinary Cargo integration, runtime-aware I/O/synchronization, process-owned state and owned messages. First workload: long-lived stateful services.

**Investigation date: 2026-10-07. This repository contains research, two working Rust mechanism probes and pinned BEAM reference fixtures, not a production runtime or a published crate.**

## What we found

**Two mechanisms now have executable evidence; the full library remains unproven.** Signals demonstrate native preemption. Generated code demonstrates automatic scheduling and safe cleanup of a killed process in a closed research subset. Extending that subset into a useful, enforceable process model is the decisive remaining gate. See [the final recommendation](docs/findings.md) and [forced-termination analysis](docs/research/05-forced-termination.md).

The accepted research direction is a **Cargo-generated native process model**: mandatory checkpoints, checked operations/types, owned state/messages, and runtime-controlled cleanup. The second [mechanism probe](experiments/generated-process/README.md) stops an infinite process after a generated checkpoint, reclaims its tracked 4 KiB buffer, and lets its peer finish; all ten unsupported-code fixtures are rejected. It is not a wrapper accepting unrestricted native Rust functions. [Model and tradeoffs](docs/generated-process-model.md).

**The core native behavior is possible, and we reproduced it.** Using pinned `open-coroutine-core` 0.7.0, two native Rust computations each performed 100 million iterations on one task-worker OS thread. Both progressed before either finished, with 25 observed task-to-task changes and no yields/awaits in their loops. Disabling preemption made the same source run serially. Both builds produced identical results. A helper thread supplies scheduling interrupts; it does not execute the computations. [Probe, reproduction and limitations](experiments/native-preemption/README.md).

**There is no native preemptive scheduler to carve out of the old lab.** Its demonstrated preemption comes from Wasmtime; Tokio and Kameo provide the surrounding execution/actor services. The valuable extraction for this project is its behavior requirements, conformance scenarios, and separation between resident application state and runtime services. Bream's native engine requires new work or adoption/hardening of a native upstream implementation. [Source audit](docs/research/01-lab-extraction-audit.md).

**A standalone library boundary makes sense.** Own native process execution, continuation lifetime, preemption, parking/wakeup, completion, and scheduling metadata. Keep actors, supervisors, distribution, persistence, and hot upgrades above it. A regular Rust program should be able to use it for two CPU jobs without adopting an actor framework. [Proposed design](docs/native-design.md).

**The hard part is compatibility and safety.** Interrupting a loop is established. Interruption around allocation, thread-local state, locks, signals, unwinding and native dependencies needs a defined contract and further evidence. Native processes also do not automatically inherit BEAM's tiny heaps or failure isolation. [Native mechanisms](docs/research/02-native-mechanisms.md), [candidate audit](docs/research/03-runtime-landscape.md).

## Planning the next experiments

The accepted first scope is **BEAM/ERTS process primitives in Rust**: processes, mailboxes, send/receive, monitoring, linking and lifecycle control. Messages use typed Rust protocols, growing mailboxes and selective receive with timeout; links follow BEAM-style exit/trapping rules. The reference workload is a room/session example built directly from those primitives. GenServer and supervisor frameworks are outside this initial scope. [Accepted scope](docs/adr/0004-beam-primitives-first.md), [revised experiment roadmap](docs/roadmap.md), [BEAM versus OTP boundary](docs/research/10-beam-versus-otp-boundary.md).

The user also selected one runtime mailbox envelope for typed payloads and lifecycle notifications, runtime-managed immutable byte sharing, and `Down` after execution stop with tracked reclamation separately observable. Observable primitive behavior should follow BEAM by default, including ordinary sends silently discarding messages addressed to dead processes.

The local [BEAM source map](docs/beam-source-map.md) now traces the actual scheduler, interpreter/x86 JIT, mailboxes, signals, spawn/relationships, exit cleanup, memory, native work, timers and I/O at a pinned OTP commit. The original traces and edge-case audits are extended by [deeper process reports and reference observations](docs/process-runtime-inventory.md), including a complete inventory of the 21 declared control-signal kinds in live/exiting dispatch. Each distinguishes traced branches from partial coverage and unproven properties. Bundled ZIP/compression/crypto algorithms are outside the port. The [latest reference pass](docs/research/29-pinned-reference-observations.md) builds pinned OTP29.1.1 and tests targeted cases on release JIT and debug interpreter VMs, including a reproduced marker assertion with controls. Bream’s future implementation experiments remain proposals. Open Rust-specific decisions are recorded in the [design tree](docs/design-tree.md).

**Behavior coverage is incomplete.** The [coverage audit](docs/behavior-coverage.md) separates known BEAM contracts, allowed race outcomes, unresolved Rust mappings and unexecuted conformance tests. Its edge-case inventories supplement the source walkthrough; they are not a claim that every case has been specified or tested.

The ownership depth pass traces [buffered send versus death](docs/research/30-buffered-signal-publication-protocol.md), [receive parking and wakeup](docs/research/31-receive-parking-wakeup-protocol.md), and [unlink/demonitor cancellation](docs/research/32-relationship-cancellation-protocol.md). Two [finite interleaving models](docs/research/33-finite-ownership-interleavings.md) exercise lifetime handoff and direct-message parking; five deliberately broken variants produce concrete counterexamples. These are bounded protocol models, separate from the executed BEAM fixtures and Rust mechanism probes.

The [comprehensive build plan](docs/build-plan.md) organizes this work into **40 feature groups and 500 numbered branch cases**, with implementation tasks, dependencies, allowed race outcomes, forbidden results and qualification gates. Detailed plans cover admission/conformance, execution/resources, messaging, lifecycle and [deeper process-runtime branches](docs/build-plan/process-runtime-expansion.md). The catalogue distinguishes initial refinements from later features and representation-specific reference research. Every feature remains planned; unresolved mappings are visible work items rather than assumed behavior.

The [performance investigation](docs/performance-plan.md) now contains **85 planned experiments**. The newest [four reports](docs/process-runtime-inventory.md) deepen thread progress/reclamation, process-lock handoff and clock/deadline semantics, adding 26 cases and 15 tasks. Seven isolated Loom invocations cover three small protocol families: three positive variants complete and four deliberately weakened controls fail as expected. The [Rust opportunity map](docs/rust-opportunities.md) adds typed access/clock domains, guard boundaries and qualified synchronization reductions. No Bream safety qualification or speedup is claimed.

## Start here

1. [Build plan and feature inventory](docs/build-plan.md): work order, 500 branch cases, edge conditions, evidence requirements and explicit later scope.
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
| Full safe process API and owned-message/resource model | Required, unproven; no production API or mailbox implementation yet. |
| Tiny native process density | Unmeasured. Wasm density from the old lab does not establish it. |
| Source-lab results | Retained evidence reviewed and final input hashes checked; two existing compiled unit tests rerun. Full lab qualifications were not rerun. |
| BEAM, Go and native candidate claims | Primary-source review, with version/source links in reports. |
| Local BEAM/ERTS implementation review | Pinned source, protocol traces and 21-kind control-dispatch inventory. Exact-version core build and targeted fixtures now executed; full suite and Bream conformance remain unexecuted. |
| Finite ownership/wakeup models | Two small sequentially consistent baseline graphs explored; five broken variants detected. No weak-memory or complete-runtime proof. |
| Bream public runtime | Not implemented; API snippets are proposals. |

Source laboratory: `/home/bat/projects/rust-actor-runtime-lab`, commit `f78658b08faaea84b7c9e7bb3a2dab13fafc17c7`. Its source was left unchanged. The local project is on Git branch `codex/research`, with no remote or published package.

The user selected **Bream** as the working name on 2026-10-07: BEAM with an R for Rust. Source, crates, macros, commands, and project paths use Bream. [Historical evidence](evidence/archive/pre-bream/README.md) preserves the original logs and tested artifacts; current results were regenerated after the rename.
