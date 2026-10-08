---
status: accepted
---

# BEAM primitives before OTP behaviours

Bream’s first scope is the native process runtime: automatically scheduled processes, process-owned mailboxes, messaging and receive, monitoring, linking, and safe process termination, expressed through Rust APIs. The user explicitly separated these BEAM/ERTS primitives from OTP behaviours and excluded GenServer, supervisors, and their restart policies from the initial scope; the room/session example will exercise raw processes directly. This keeps the runtime independently useful and allows higher-level libraries to be built later without making an OTP framework a prerequisite.
