# Static payload cost experiment — results removed

**Performance evidence removed, 2026-10-08, at the user’s request. All speed tests must be redone in an authorized suitable environment. No speed tests may run on this machine.**

The former timing samples, summaries, ratios, verdicts and disassembly artifacts have been deleted. No speed improvement or regression is established for PERF-091. The [experiment source](../../experiments/static-payload-costs/main.rs) and [future protocol](../../evidence/static-payload-review/protocol.md) remain available for designing a new run.

The source-level question remains useful: compare owned `Arc<[u8]>` storage, truly static immutable bytes, and a concrete static/shared representation under the same ownership and release contract. Type shape, reference-count operations, allocation and destruction are separate obligations. Fewer operations alone do not establish a faster workload.

The [literal-lifetime source review](51-code-literal-retirement-and-rust-lifetimes.md) remains valid source analysis. Keep its lifetime and process-admission constraints when redesigning the experiment. Future qualification must include useful application work, peer progress, safe stop and final cleanup, according to the [performance plan](../performance-plan.md).

[Removal record and correctness audit](../../evidence/performance-reset/README.md).
