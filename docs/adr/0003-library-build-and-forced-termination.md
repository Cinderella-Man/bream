---
status: accepted
---

# Ordinary Cargo integration and safe forced termination

The user requires an ordinary Cargo dependency with the standard Rust compiler for the first version, and considers safe forced termination essential. Processes should own their state and communicate through owned messages; termination should reclaim runtime-managed resources without promising execution of arbitrary user destructors. These are product constraints, not claims that the complete contract has been implemented: the signal probe proves pause/resume scheduling, while the generated-process probe adds termination and cleanup only for its closed subset. Custom-compiler execution, cooperative-only stop requests, indefinite resource retention, or OS-process isolation must not silently substitute for the requested contract.
