# Process I/O ownership and the native-library boundary

Deeper source review, 2026-10-07. Reference: OTP `29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. This report traces process-facing group-leader and native readiness ownership paths. It does not port a socket library, audit every polling backend, or execute a test.

The user's boundary is explicit: compression, cryptography and other bundled application libraries are not Bream implementation targets. Rust libraries can supply those algorithms. What Bream must own is their interaction with process scheduling, lifetime, messages and resources. A native dependency's existence does not itself establish that its longest call can be interrupted and safely stopped.

## Group leader is inherited I/O routing metadata

BEAM gives each process a group leader, inherited from the spawning process, through which the Erlang I/O protocol is routed. This is not a link, monitor, supervisor or automatic parent-owned lifetime. The data and primitive are runtime facilities; the surrounding I/O request conventions are library protocols. [Public contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L3275-L3323).

The local setter tries a locked direct update only when signal order permits it; otherwise the Erlang wrapper sends a request and waits for its correlated reply. A nonexistent/exiting target fails. The native setter checks that the new leader is a PID, not that the leader remains alive; do not infer successful future I/O from assignment. When the target is running dirty code, the PID is stored in a heap fragment because that target's heap cannot safely be written. Remote setters have separate connection/send handling and cannot be treated as a synchronous local acknowledgment. [Wrapper](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L3324-L3346), [native branches](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L4852-L4968).

Bream may ultimately expose a Rust I/O capability rather than reproduce the Erlang I/O protocol. That would require an explicit mapping. For now, inherited routing and owner-independent process lifetime belong in the research inventory; group-leader APIs are not silently required by the first room fixture.

## Readiness notification, cancellation and safe closure differ

The traced native path is `enif_select[_x] → per-FD state/poll registration → readiness dispatch → send_select_msg`, with stop and cancellation branches sharing the registration state. This is one concrete reference for a Rust resource reactor, not a proposal to embed NIFs.

| Branch | Inspected behavior |
|---|---|
| Register read/write/error interest | Validate platform/event support, hold the per-FD lock, choose add/modify, associate the resource and retain an owning reference. |
| Replace an existing registration | Distinguish the same resource from another resource, a driver registration or pending stop. The latter paths are not ordinary overwrite of a callback pointer. |
| Registration fails | Return a failure flag and restore relevant state rather than exposing a partially operational event. |
| Readiness arrives | Detach the selected message and clear its stored PID/message pointer under the lock; send afterward. Notifications are one-shot and must be rearmed. |
| Recipient disappeared | Release the unsent notification. In scheduler-poll mode, also clear its internal select state when signal delivery fails. |
| Cancel a pending direction | Remove only an event still stored as pending and report the corresponding cancelled flag. An already-detached notification may have been sent. |
| Stop while closure is immediately safe | Invoke the stop callback and release the retained resource ownership in the direct path. |
| Stop while polling may still use the event | Preserve the resource and schedule the stop callback after poll coordination; only then may the OS object be safely closed. |

Source: [registration, validation and ownership conflicts](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/common/erl_check_io.c#L1236-L1340), [failure/cancellation/stop branches](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/common/erl_check_io.c#L1421-L1541), [readiness ownership transfer](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/common/erl_check_io.c#L2121-L2180), [dead recipient cleanup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/sys/common/erl_check_io.c#L836-L874).

The documented contract requires the stop protocol even after every notification has been received or cancelled. A resource with active event associations cannot be destructed until those associations are dissolved. Detecting process death may need a resource monitor so that a dead user process does not leave native registrations alive forever. Therefore “the process died,” “interest was cancelled,” and “the descriptor may be closed/reused” are three separate facts. [Readiness lifetime contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L3566-L3618).

The native tests exercise cancel-before-notification versus cancellation after readiness, alternative recipients, re-registration and stop flags. These fixtures are useful scenario sources; they were inspected, not executed. [Select tests](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/nif_SUITE.erl#L908-L1016).

For Bream, a native readiness registration, queue node and pending completion need separate tracked ownership. A stale numeric file descriptor cannot safely identify a new registration after descriptor reuse. The target's application work remains stopped after `Down`; pending runtime I/O bookkeeping may continue only while preserving the resources it still uses. The exact Rust backend, completion APIs and resource types remain unselected.

## Utility libraries are consumers, not things to port

BEAM itself illustrates the boundary. `enif_consume_timeslice` charges reductions and reports exhaustion; it does not interrupt a running native function. Its dirty-scheduler branch is a no-op. The scheduled normal-NIF wrapper calls the native function and regains control when it returns. Moving a function to dirty workers changes scheduler contention, not the native function's forced-termination contract. [Accounting](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3341-L3359), [native call/return](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L3491-L3530).

Accordingly, the implementation catalogue distinguishes:

1. **Runtime mechanisms:** automatic scheduling, state lifetime, messages, relationships, timers, resource registration and reclamation.
2. **Approved process operations:** Rust helpers whose execution, partial state and destruction fit the admitted model. These may reuse existing library code where its contract is qualified; adding a helper must not silently bypass preemption or kill.
3. **Application libraries:** compression, cryptography, codecs, HTTP clients and similar capabilities. Bream does not reimplement their algorithms. Unqualified calls remain outside the guaranteed process path until the integration problem is solved.

This does not select an external-job escape or loosen the user's safe-kill requirement. It preserves the user's library boundary while making the native integration obligation explicit.

Resource destruction itself is native work. ERTS schedules NIF resource destruction as auxiliary work in a suitable lock context, and tracks outstanding resource-monitor references before final release. That proves a lifetime protocol exists; it does not prove arbitrary callback bodies are short, preemptible or harmless. Rust `Drop` implementations need the same scrutiny at admission. [Destructor prologue and monitor coordination](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_nif.c#L2961-L3005).

## Additional branch cases

IO-01 is extension research; native-resource ownership obligations already constrain EXEC-03/LIFE-05 whenever such resources are admitted.

| Case | Branch and required qualification |
|---|---|
| IO-01.C01 | Spawn inherits I/O routing; change routing on self/live/busy/exiting target; preserve signal ordering and distinguish successful assignment from leader liveness. |
| IO-01.C02 | Register/rearm read/write interests, replace recipient/reference or encounter another owner's registration; define ownership and prevent stale-generation delivery. |
| IO-01.C03 | Readiness is pending, detached, delivered or consumed when cancellation occurs; cancelled flags must match the actual boundary, without flushing unrelated mailbox items. |
| IO-01.C04 | Stop callback can run immediately or only after poll coordination; do not close/reuse the OS handle before safe closure. |
| IO-01.C05 | Recipient dies with registrations or detached notifications outstanding; release notification ownership and eventually stop native registrations without post-death application execution. |
| IO-01.C06 | Registration allocation/platform/poll failure; establish the post-failure interest/ownership state and release abandoned work. Distinguish unsupported capability from successful empty readiness. **Refined by [report42](42-linux-readiness-and-registration-lifetime.md):** a failed operation does not universally restore the prior registration; the ADD/MOD/DEL and fallback branches require separate oracles. |
| IO-01.C07 | Resource callback/destructor or an ordinary Rust dependency performs unbounded work; fail admission or qualify a resumable path, never count dirty-worker offload as safe kill. |
| IO-01.C08 | Runtime shutdown races readiness/cancel/stop and descriptor reuse; retain required memory until the final owner completes, and expose truthful cleanup completion. |

Remaining coverage: OS-specific epoll/kqueue/Windows branches, full port task scheduling, every resource monitor/destructor race, and concrete Rust reactor cancellation. Those gaps are listed rather than hidden behind a claim that a whole I/O subsystem has been audited.

Plan integration: these case IDs are recorded in the [expanded build catalogue](../build-plan/process-runtime-expansion.md). All remain planned, with conditional applicability preserved.
