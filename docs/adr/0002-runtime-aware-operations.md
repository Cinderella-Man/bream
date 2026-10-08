---
status: accepted
---

# Runtime-aware operations in the first native profile

The first implementation may require Bream-aware channels, synchronization, and I/O, and may target Linux x86-64 first; the user accepted both boundaries on 2026-10-07. This trades transparent use of every blocking standard-library operation for a tractable native scheduling contract while retaining automatic preemption of CPU loops. It does not settle allocator/TLS safety, compiler requirements, cancellation semantics, or eventual portability.
