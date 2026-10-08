---
status: accepted
---

# Native Rust processes with runtime-enforced preemption

Bream targets a standalone Rust library that suspends and resumes CPU-heavy native Rust processes without application-written yields or awaits; the user explicitly confirmed this on 2026-10-07. A Wasm guest runtime or a cooperative executor would change the requested product, so those remain comparators rather than implementations of the requirement. The preemption mechanism, supported platforms, compiler integration, and compatibility restrictions are still open; scheduling quantum expiration must preserve the computation rather than kill it.
