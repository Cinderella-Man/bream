# Local name-routing and Rust variance observations

Executed 2026-10-08. [Report52](52-name-resolution-and-typed-routing.md) gives the source interpretation. The [runner and raw evidence](../../evidence/name-routing-review/README.md) record eight local BEAM scenario families in each of three pinned VM profiles, plus two Rust variance controls in each of two optimization profiles. **All expected observations occurred. No Bream runtime or speed benchmark was executed.**

## Exact reference and isolated runs

The existing separately built OTP `29.1.1` core, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`, was reused. The runner first checks the checkout commit, absence of tracked changes and all four retained executable/script hashes against the earlier [build provenance](../../evidence/process-protocol-review/source-build.json). Each profile compiles its own retained `.beam` file. The source checkout and earlier evidence are unchanged; the original full-application build's optional-wx failure remains as previously documented.

Profiles are release JIT with one and four schedulers, and debug interpreter with four schedulers. Both dirty pool sizes are set to one. Each child command has a 45-second external watchdog and core dumps disabled; message waits inside the fixture use three-second failure bounds. These bounds protect the harness, not product latency promises. Environment Erlang flag/compiler-option overrides are cleared and recorded. [Manifest](../../evidence/name-routing-review/probe-results.json)

| Family, in all three profiles | Required observation and setup |
|---|---|
| Cached versus dynamic address | Capture A's PID; unregister its name and bind B; cached PID delivers to A while atom and local tuple deliver to B. Kill A and verify B's binding remains. |
| Named monitor incarnation | Establish name monitor on A, rebind to B, kill A. Named DOWN reports A's termination while B stays registered/alive. This also exercises unregister by a process other than the target. |
| Initially missing monitor | Monitor absent name, then register B before consuming DOWN. Result remains `noproc`; B is unaffected. It does not assert when the notification was queued. |
| Missing address forms | Absent atom send raises `badarg`; absent `{Name, node()}` send and a send to a confirmed-dead PID complete with discard. `send/3` with `noconnect` also completes for the absent local tuple. No remote connection is used. |
| Detached name snapshot | Obtain `registered()` containing the fixture name, then unregister. The retained list still contains that name while current lookup returns `undefined`. |
| Timer late binding | Create a 250-ms name timer for A's binding, rebind to B and require a strictly positive `read_timer` result after rebinding. B then receives the message. An expired setup fails; it is never counted as success. |
| Competing names | Fifty rounds: two registrars race different names for one live target. Exactly one succeeds; only the winning name resolves to that target. Death clears both fixture-name lookups. |
| Competing targets | Fifty rounds: two registrars race one name for two live targets. Exactly one succeeds and resolves to its winner. Both targets are subsequently killed and name absence checked. |

Total: **24 family/profile observations and 300 race rounds**. The two race families check outcomes, not scheduler diversity; even the four-worker run does not prove that all interleavings occurred. The sequential rebinding tests do not force death exactly between lookup and publication. No sorting guarantee, complete registry linearizability proof or port behavior is established.

## Rust: a constructor check can be bypassed by covariance

The positive counterexample stores `TypeId::of::<fn(&())>()` in a token containing `PhantomData<T>`. Safe Rust then coerces that token to the corresponding `fn(&'static ())` specialization without running another constructor. The stored type identity differs from the new specialization's identity. This demonstrates why a generic token's constructor check is not, by itself, a proof that every later specialization was checked. It executes no unchecked casts or payload delivery.

The companion token uses `PhantomData<fn(T) -> T>` and the same attempted conversion. Compilation fails with the expected mismatched-types diagnostic. These are isolated controls for variance in these concrete type shapes, not proof that every invariant marker design is a sound registry. [Official TypeId variance warning](https://doc.rust-lang.org/std/any/struct.TypeId.html#danger-of-improper-variance)

Rust `1.98.1`, edition 2021, optimization levels 0 and 3: **four compilation attempts**, two successful counterexample executions, two expected compile rejections. Compiler, runner, input, executable and raw-stream hashes are retained. The currently consulted online Rust docs identify a newer toolchain; the executed compiler version is independently recorded. These observations support private typed construction and variance review; they do not provide process admission, preemption, cleanup or weak-memory evidence.

## What remains to test

Implement no optimization solely from these observations. A typed registry needs a concrete protocol/rebinding policy, runtime-instance identity, record-retention rules and tests for the check/use interval. Name-token reclamation, shard transactions, coherent concurrent enumeration, cache invalidation and timer delivery across incompatible rebindings still require implementation-level controls. Dynamic code retirement and system-task source reports in this pass have no new executed reference fixtures.
