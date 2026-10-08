# Bream

**Status: selected working name.** On 2026-10-07 the user chose Bream “for the time being” and requested the project be renamed throughout.

**BEAM with an R for Rust.** Bream is short, hints at the runtime’s inspiration, and carries a fish joke without making the API vocabulary unusual. The project is Bream; the crate/API spelling is `bream`, as in proposed `bream::spawn` and `#[bream::process]` interfaces. Those interfaces remain proposals, not a published library API.

Suggested line: **Small processes. Fair turns.** Public types should use conventional names such as `Process`, `ProcessId`, `Runtime`, and `Exit`.

The project lives at `/home/bat/projects/bream`. Research probe crates are `bream-native-preemption-probe`, `bream-probe-macros`, and `bream-probe-runtime`.

## Registry check

The exact crates.io lookup for `bream` returned HTTP 404 on 2026-10-07. This means no crate was returned at the endpoint when checked, not that the name is reserved or publication is guaranteed. [Recorded registry checks](../evidence/name-checks-beam.json), [registry endpoint](https://crates.io/api/v1/crates/bream).

No crate, remote repository, domain, or account was registered. Earlier naming notes, rejected candidates, and original experiment artifacts are preserved in the [historical archive](../evidence/archive/pre-bream/README.md).
