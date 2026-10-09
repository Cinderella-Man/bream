# Explicit self-exit and targeted local exit: source contract

Research date: **2026-10-09**. Sources: pinned **OTP-29.1.1**, commit **`ad05823719d77c8faee87348ea39513d4e2f99c5`**. This is a source audit for the next bounded implementation milestone; no builds, experiments or performance measurements were run for this document. It refines the existing [lifecycle edge cases](16-lifecycle-edge-cases.md), [exit cleanup](20-process-lifecycle-deep-dive.md) and [signal dispatch inventory](24-process-signal-dispatch-matrix.md).

The essential distinction is between **raising an exit exception in the current process**, **sending an explicit exit signal**, and **propagating an exit through a link**. The reason alone cannot identify the operation. In particular, `kill` changes to `killed` only on receipt of an explicit kill signal; an uncaught `exit(kill)` terminates with `kill`, which remains trappable when propagated through links.

The subsequent [implementation findings](65-explicit-exit-milestone.md) record the passed restricted profile and executed reference controls separately from this source audit.

## Bounded semantic matrix

In these tables, `R` is an admitted reason other than `normal` or `kill`. Both `killed` and `noproc` belong to that ordinary abnormal category. “EXIT message” means `{'EXIT', Sender, Reason}`. References are ordinary local PIDs; aliases, ports, distribution and priority messages are outside the milestone.

| Operation | Receiver traps exits? | Result |
|---|---|---|
| Uncaught `exit(R)` in current process | Either | Current process terminates with exactly `R`. |
| Uncaught `exit(normal)` | Either | Current process terminates normally. |
| Uncaught `exit(kill)` | Either | Current process terminates with `kill`, not `killed`. |
| `exit(Other, normal)` | No | Target ignores the signal. |
| `exit(Other, normal)` | Yes | Target receives an EXIT message with `normal`. |
| `exit(Other, R)` | No | Target terminates with `R`. |
| `exit(Other, R)` | Yes | Target receives an EXIT message with `R`. |
| `exit(Other, kill)` | Either | Target terminates with `killed`; this signal cannot be trapped. |
| `exit(self(), normal)` | No | Caller terminates with `normal` before the call returns. This is a legacy special case. |
| `exit(self(), normal)` | Yes | Caller remains alive and receives an EXIT message from itself. |
| `exit(self(), R)` | No | Caller terminates with `R` before the call returns. |
| `exit(self(), R)` | Yes | Caller remains alive and receives an EXIT message from itself. |
| `exit(self(), kill)` | Either | Caller terminates with `killed` before the call returns. |
| `exit(DeadPid, Reason)` | Not applicable | Returns `true`; no target action and no automatically generated `noproc` reply. |

`exit/1` itself is a **catchable exception**, even with reason `kill`; it is not an unconditional process-kill primitive. The matrix deliberately scopes its self-exit rows to an uncaught exception. `trap_exit` governs received exit signals and does not catch a locally raised exception. `exit/2` does not raise a catchable exception in its target. In particular, wrapping a fatal `exit(self(), Reason)` in `catch` does not save the caller. [Exit BIF and exception class](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1533-L1540), [documented exception and kill propagation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L2470-L2508), [upstream catch regression case](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L381-L393).

The direct-signal rows come from the BIF's self-target handling and the receiver's signal policy. The BIF marks legacy self-suicide when sender and target match and either the reason is `kill` or trapping is disabled. It queues the signal, fetches incoming signals and enters `await_exit` instead of returning. The receiver then either converts the signal into a message, ignores normal, or commits exit. [Self-target BIF path](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1747-L1773), [receiver policy](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4738-L4781).

**Version detail:** OTP 29 introduces `exit_signal/2` and deprecates the old `exit/2` spelling. The new operation has the same ordinary direct-signal policy but does not implement the legacy self-normal exception: `exit_signal(self(), normal)` is ignored when the caller does not trap exits. Bream must name its chosen contract; it must not silently claim both behaviors. The implementation milestone's legacy `exit/2` comparison needs an explicit self-normal test. [Pinned API documentation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L2512-L2597), [different BIF mode argument](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1872-L1885).

## Linked propagation preserves the committed reason

| Source's committed reason | Active linked receiver without trapping | Active linked receiver with trapping |
|---|---|---|
| `normal` | Ignores the exit. | Receives EXIT with `normal`. |
| `kill` | Terminates with `kill`. | Receives EXIT with `kill`. |
| `killed` | Terminates with `killed`. | Receives EXIT with `killed`. |
| `noproc` or other admitted abnormal reason | Terminates with the same reason. | Receives EXIT with that reason. |

On process exit, local link traversal sends the committed reason with opcode `EXIT_LINKED`. The receiver checks that the corresponding relationship still exists and is not unlinking, removes that link, and then applies exit policy. A deactivated link causes the signal to be dropped. In contrast, an explicit `EXIT` signal needs no link and does not itself remove an existing link when trapped or ignored. [Local teardown sender](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14065-L14084), [distinct direct and linked opcodes](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L2514-L2598), [active-link validation and removal](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4674-L4723).

This gives two different chains:

- `A` executes uncaught `exit(kill)` → `A` dies with `kill` → an untrapping linked `B` dies with `kill` → a trapping linked `C` can receive EXIT with `kill` and survive.
- `X` sends explicit `kill` to `A` → `A` dies with `killed` → an untrapping linked `B` dies with `killed` → a trapping linked `C` receives EXIT with `killed` and survives.

The signal handler checks both opcode and reason before bypassing trapping or rewriting the reason. OTP's own `kill2killed` regression test covers direct, self-directed and `exit/1` cases through link chains. [Receipt implementation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4738-L4770), [upstream chain fixture](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/signal_SUITE.erl#L187-L247).

## Receipt, ordering and completion are separate obligations

The receiver evaluates its current `trap_exit` flag when handling the signal. A Bream implementation must not decide the eventual outcome by snapshotting the recipient's trapping flag on the sender side. A controlled state transition can therefore change the outcome of an already queued signal. [Receipt-time flag test](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4738-L4742).

Signals from one sender to one recipient preserve their order. This covers ordinary messages and explicit exit signals together. For a trapping target with ordinary message reception, `before`, explicit abnormal exit, `after` become the corresponding message sequence. For an untrapping target, a fatal exit can terminate it before application code consumes an earlier message; signal order is not a guarantee that the application executes all earlier messages. The important prohibition is application observation of a later same-sender message after the preceding fatal exit took effect. [Language ordering rule](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L495-L511), [ordinary queue insertion order](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L345-L359), [sender identity used for exit ordering](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L1830-L1840), [upstream message/exit ordering test](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/signal_SUITE.erl#L155-L185).

The legacy self-suicide path preserves earlier signal order by processing incoming signals until exit commits, yielding if necessary. Replacing it with an unconditional reason store could discard an earlier fatal signal and commit the wrong reason. After exit commits, the live dispatcher stops; the exiting dispatcher releases later exit representations instead of replacing the reason. [Self-suicide signal drain](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1698-L1716), [stop live handling on exit](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6338-L6353), [late signal destruction](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7311-L7340).

For another target, `true` means the exit-send operation returned; it does not prove delivery, target death, reclamation or a specific terminal reason. If local PID lookup fails, the queued signal representation is freed. No direct-exit `noproc` reply is manufactured. Failed monitor/link establishment has its own `noproc` protocol and should not be copied onto direct exit. [Absent process lookup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L1147-L1178), [failed-send cleanup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L1830-L1840), [BIF return](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1763-L1772).

A monitor established before termination reports the process's committed reason. A later monitor can instead report `noproc`, because it observes absence rather than a historical result. Tests requiring the exact explicit-exit reason must establish the monitor first. Monitors and links use the same exit context's reason, but arbitrary notifications from different dying processes have no total ordering. [Monitor exit callback](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13795-L13816), [link callback](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14065-L14084), [absent monitor target](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L1138-L1144).

## Implementation implications for Bream

These are design consequences of the audit, not claims that the next implementation has already passed:

1. **Use distinct operations.** A checked terminal `self_exit(Reason)` can model the uncaught part of `exit/1` without admitting arbitrary Rust exceptions or pretending to provide Erlang's catch semantics. Targeted exit needs a separate signal kind from linked propagation. The bounded reason enum must distinguish `Kill` from `Killed`.
2. **Keep targeted exit in the shared signal order.** Message sends, direct exits and admitted relationship events need one coherent per-sender order. A separate high-priority kill queue would change the observable contract unless explicitly specified as a different host operation.
3. **Preserve exact identities.** A signal addressed to an old generation must never act on its replacement. An absent target consumes the request without a fabricated Down or Exit. The current Bream generation/namespace mechanism remains relevant even though Erlang's PID representation differs.
4. **Commit once, then clean up.** Fatal self-exit and targeted exit must converge on the existing terminal arbitration and bounded cleanup path. No application statement after committed self-exit may execute. Later requests must not replace the reason or emit duplicate relationship notifications.
5. **Separate delivery from reclamation.** Existing monitor/exit observations can establish logical death, while held buffers, queued shared references and any supported pending operations need separate cleanup receipts. This keeps the [green-runtime lessons](63-rust-green-runtime-lessons-and-experiments.md) relevant without broadening this milestone into native I/O.

For responsiveness, the useful structural opportunity is to reuse one closed enum, one terminal transition and the existing bounded control/cleanup service rather than introduce unbounded per-exit callbacks or native unwinding. This is an implementation hypothesis; the source audit makes no speed or latency claim.

The planned restricted API uses `self_exit` for the uncaught self-exit contract and `exit_to` for the legacy targeted contract, with reason codes `Normal=1`, `Killed=2`, `Failure=3`, `NoProcess=4`, `Kill=5`. Its direct-signal receipt is performed synchronously under registry/lifecycle arbitration, with the current trapping state read at that point. This is an eager local delivery model, not a reproduction of OTP's signal backlog. It must preserve ordering against already published events and must not claim to exercise a delayed direct-signal receipt race that its interface does not expose. Linked-signal conversion remains a separate operation.

## Required bounded controls

| Control | Observation to require |
|---|---|
| Uncaught self-exit for every admitted reason with both trap settings | Exact reason, no following application report, one Down for a pre-established monitor. |
| Catch distinction in the BEAM reference | `catch exit(kill)` returns a caught exit; `catch exit(self(), kill)` does not save the caller. The restricted Rust profile documents that it has no catch operation. |
| Targeted matrix with and without trapping | Normal ignored or delivered; ordinary abnormal reasons delivered or fatal; explicit kill always fatal as killed. |
| Legacy self-target matrix | Self-normal without trapping terminates, self-normal with trapping is delivered, and self-kill becomes killed. |
| Direct kill versus propagated kill/killed | Chain tests preserve reason and allow the trapping endpoint to survive. |
| Same-sender message/exit/message | Exact three-event order for trapping target; no later application message observed by a fatally exiting target. Do not require consumption of earlier messages before death. |
| Trapping state at receipt | Current state decides the outcome; vary publication/receipt interleavings where that signal path exposes separate stages. An eager direct-receipt path documents that limit. |
| Explicit exit between linked peers | A trapped direct exit leaves their link active; subsequent source death still produces linked Exit. |
| Dead PID and forced slot reuse | Requests to the old identity are harmless and do not notify or terminate the replacement. |
| Competing terminal events | One committed reason, one notification per active relation, no second terminal transition. |
| Held/queued immutable bytes and shutdown | Survivor data remain valid; request, mailbox and actor-owned references are released exactly once through existing cleanup. |

Run finite assertions and protocol handshakes only. Watchdogs may detect hangs; neither their duration nor completion order across unrelated senders is performance evidence. Broader exception catching, dynamic reasons, priority aliases, distribution, native code cancellation and multiple-worker qualification remain outside this milestone.
