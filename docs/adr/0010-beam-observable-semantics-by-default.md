---
status: accepted
---

# Follow BEAM observable semantics by default

The user instructed Bream to behave as closely as possible to BEAM/Elixir, so ordinary local process semantics should be derived from the pinned BEAM reference rather than repeatedly offered as independent API preferences. In particular, ordinary send to a dead process silently discards the message; send does not acknowledge consumption, and the runtime remains responsible for safely releasing the moved payload. Rust-specific representation and safety differences must be explicit, preserving the already accepted typed envelope, ownership model and stop-before-Down guarantee rather than adopting native-extension exceptions implicitly.
