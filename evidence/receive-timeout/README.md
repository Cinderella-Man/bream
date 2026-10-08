# Receive-timeout correctness evidence

This [milestone](../../experiments/receive-timeout/README.md) uses injected logical time. No speed test, benchmark, latency measurement or performance comparison ran. Watchdogs detect hangs only; incidental Cargo duration text is not analyzed. Builds use two jobs and the pinned Rust 1.98.1 toolchain. [Standing machine constraint](../../AGENTS.md).

The correctness-only [runner](run.py) writes a protocol before copying inputs and executing each uniquely named cohort. It retains exact source, tool and executable identities, commands, raw streams and outcomes. Existing names are refused. No historical combined benchmark runner or validator is invoked.

| Cohort | Interpretation |
|---|---|
| [01](runs/20261008-timeout-01/results.json) | Retained build failure: rustfmt removed braces around a nested match arm, which the checker rejected. |
| [02](runs/20261008-timeout-02/results.json) | Commands passed, but review found the new bare nested-match path omitted one generated checkpoint. Superseded; not the qualified implementation. |
| [03](runs/20261008-timeout-03/results.json) | Added a public logical work-count regression before fixing the checker. The braced and formatted forms expose different charges; retained failing control. |
| [04](runs/20261008-timeout-04/results.json) | Final source restores the checkpoint on the recursively checked nested-match path. Current qualification cohort. |

The final protocol tests 35 internal cases in debug and release, three inherited mailbox configurations and three timeout configurations in each profile, 75 expected rejected consumers per profile and four positive shadowing programs per profile. Six timeout invocations contain 96 send/timeout and 96 kill/timeout races; six inherited mailbox invocations retain 96 send/kill races. These finite observations do not exhaust all interleavings.

Timeout controls intentionally provoke two process panics: invalid finite duration and checked deadline overflow. Their raw stderr is expected; the worker catches each failure and continues, with ownership released and locks still usable. A passing result also requires no remaining process timer registration; an empty deadline index alone is insufficient.

Reproduce another bounded correctness cohort from the repository root:

```sh
python3 evidence/receive-timeout/run.py UNIQUE_NEW_CORRECTNESS_RUN_NAME
```

The [validator](validate.py) now delegates to the [performance-reset audit](../performance-reset/README.md). All four timeout cohorts and their source snapshots remain, including the failed regression. Earlier speed evidence was deleted at user request and historical hash inventories were rebased. Old validation outputs are explicitly superseded; no runtime or benchmark is executed by this audit.

```sh
python3 evidence/receive-timeout/validate.py
```

Clock publication is not an expiry-service barrier. Timeout precedence assertions therefore use explicit callback commitment in deterministic tests. Public races permit either supported winner. The runtime has no real-time timer driver; no BEAM execution, production latency or broader G3 completion is claimed.
