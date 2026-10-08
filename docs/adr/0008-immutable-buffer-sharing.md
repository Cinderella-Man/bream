---
status: accepted
---

# Permit runtime-managed immutable byte sharing

Bream may share runtime-managed immutable byte storage across process lifetimes while each process owns its reference, avoiding repeated payload copying during room fanout. The user selected this narrow sharing model over moving or copying every payload; shared mutable application state and arbitrary shared Rust types remain outside the accepted ownership contract. Process cleanup releases its references without invalidating live recipients, and final-owner reclamation and physical storage accounting must be qualified.
