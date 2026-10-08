# All declared process-control signal kinds: live and exiting dispatch

Pinned OTP `29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`; inspected 2026-10-07, no execution. This report makes one deliberately finite coverage claim: **all 21 declared non-message signal operation kinds were matched to the main live-process and exiting-process dispatch branches.** This is a dispatch inventory, not a claim to have exhausted every subtype, helper, backend or concurrent interleaving.

Ordinary messages are handled through the message-queue paths and are not an additional numbered operation in this list. NIF_SELECT's live branch is conditional on scheduler polling. [Operation declarations](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.h#L174-L196).

## Dispatch matrix

The live dispatcher covers [exit and monitor-down](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6338-L6514), [relationships and persistent events](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6516-L6775), and [control/housekeeping/markers](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6778-L6968). The exiting dispatcher handles [late signal operations](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7311-L7469) after removing each from the non-message queue.

| Operation | Live-process action | Exiting-process action | Deeper path |
|---|---|---|---|
| EXIT | Apply direct-exit policy; ignore, notify or enter exit | Release message/link/monitor representation according to subtype | Reports 16/20 |
| EXIT_LINKED | Apply relationship-sensitive exit policy | Release representation; do not replace committed reason | Reports 16/20 |
| MONITOR_DOWN | Validate active origin; convert/drop; handle alias survival and priority removal | Release representation without application delivery | Reports 19/20 |
| MONITOR | Install target endpoint; suspension monitor may suspend/yield | Run late-monitor exit handling with noproc context | Reports 18/20 |
| DEMONITOR | Remove matching endpoint; resource or suspension subtype performs its special release/resume | Destroy remote cancellation record or release endpoint | Reports 18/20 |
| LINK | Insert relation or release duplicate setup | Reply through link-exit handling with noproc context | Report 20 |
| UNLINK | Remove eligible local endpoint and acknowledge; preserve concurrent-unlink rules | Still acknowledge, including local/remote/port variants; do not strand initiator | Report 20 |
| GROUP_LEADER | Apply active request and return success/failure, release request shares | Same handler sees exiting state and answers failure when applicable | Report 22 |
| TRACE_CHANGE_STATE | Update tracing state through ordered handling | Destroy pending trace-state data | Report 21; helper internals partial |
| PERSISTENT_MON_MSG | Convert only while its origin monitor is present; support priority/tag/node variants | Discard message and release payload | This report; complete clock/node service still partial |
| IS_ALIVE | Send true response | Send false response | Report 21 |
| PROCESS_INFO | Gather requested fields with alive target and queue coordination | Answer absent-target result and free request | Report 21 |
| SYNC_SUSPEND | Reply now or attach reply to pending suspension | Use terminal-state suspension reply handling | Report 18 |
| RPC | Invoke a runtime control callback with work allowance, deliver result or free orphaned response | Callback still runs with exiting process context; must handle that state | This report; each callback needs its own audit |
| DIST_SPAWN_REPLY | Complete pending spawn relationship/reply protocol | Run special abandoned/exiting-requester cleanup | Report 20 |
| ALTACT_MSG | Resolve alias/priority action and convert or drop | Drop and free prepared message | Report 19 |
| RECV_MARK | Move marker to received region, complete delayed save position, or retire it | Clear location/save state and deallocate marker | Report 19 |
| UNLINK_ACK | Remove only a matching pending unlink operation; release acknowledgment | Release acknowledgment storage | Report 20 |
| ADJ_MSGQ | Continue literal-copy or off-heap conversion, with adjusted work allowance | Run the corresponding helper in exiting mode | Report 18; full literal-copy paths partial |
| FLUSH | Mark signal barrier reached, free marker, stop exclusive signal handling | Mark barrier reached and free marker | Report 21 and this report |
| NIF_SELECT | Finish select handling, convert prepared signal to message and notify process | Drop/release prepared message | Report 22; scheduler-poll-specific helper partial |

The matrix exposes an architectural requirement: entering exit cannot mean indiscriminately discarding every control signal. Late monitor/link requests need an outcome, unlink still needs acknowledgment, observation requests need a terminal answer, and ownership-bearing messages must be released. Conversely, servicing those protocols does not authorize resuming application code.

## Less-obvious branches inside this dispatcher

**Monitor-down is a family, not just a tuple constructor.** The main branch distinguishes local/remote process/port records, generic external exit payloads, node monitors and suspension monitors. It checks whether the origin still exists; excludes standalone aliases from lifetime observation; handles pending spawn differently; and either preserves an explicit-unalias capability as a standalone alias or removes it. Priority capability removal happens after notification insertion. Malformed distributed exit payload decoding can drop the signal. This is why broad “every DOWN is one-shot and removes all associated state” wording needs the ordinary-process-monitor scope. [Monitor-down branch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6355-L6514).

**Persistent events retain their subscription.** `handle_persistent_mon_msg` handles time-offset and node-monitor event forms. An active origin gates delivery; time-offset notifications may replace the tag, and repeated node-monitor subscriptions can require multiple copied notifications. Removing a subscription before signal handling suppresses conversion. This is not the same lifecycle as a process monitor's terminal Down. [Persistent handling](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L5170-L5247), [origin gate/drop](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6516-L6542). Complete time-offset/node subscriptions remain future source work.

**RPC callbacks can run during cleanup.** `handle_rpc` passes available work to an internal function, permits it to request a yield through its accounting result, then looks up the requester. If the requester died, returned heap fragments are freed; otherwise the result and correlation reference are queued. The callback's arguments and result ownership matter even when neither process will run application code again. This facility is an internal runtime hook, not a safe API for arbitrary user closures. [RPC call/result cleanup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L3564-L3622).

**FLUSH is not demonitor's mailbox flush.** This operation marks an internal signal-processing barrier. It changes `FS_FLUSHING_SIGS`/`FS_FLUSHED_SIGS` handling and can end a signal-only phase; it does not scan for a Down-shaped application message. Likewise, ADJ_MSGQ can enlarge the current signal allowance for incremental housekeeping; the numeric signal limit is not a universal maximum amount of work. [Adjustment and barrier branches](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6815-L6865).

## Cross-dispatch build obligations

SIG-01 strengthens the initial control/cleanup invariants for supported signals. Additional signal families apply only when their corresponding extension is implemented.

| Case | Branch and required qualification |
|---|---|
| SIG-01.C01 | Every admitted control kind arrives before/during/after exit transition; define response/drop/cleanup for each, without resuming application work. |
| SIG-01.C02 | A requester's death races a runtime RPC/inspection/suspension result; release request and reply ownership and avoid indefinite pending obligations. |
| SIG-01.C03 | Internal signal flush crosses exit, partial queue handling or delayed conversion; distinguish barrier completion from application-message removal. |
| SIG-01.C04 | Persistent event has active/removed subscription, custom tag or repeated node observation; preserve the correct conversion multiplicity and lifetime if exposed. |
| SIG-01.C05 | Expensive adjustment/control callback uses or extends its work allowance; qualify progress, stop boundaries and partial-state cleanup instead of assuming the dispatch count is a hard limit. |

The default dispatcher arm treats an unknown opcode as an internal error. A safe Rust runtime should make unsupported variants unconstructible through its public surface and exhaustively handle its admitted internal enum. This is a maintainability/ownership requirement, not a mandate to use ERTS's opcode numbers or reproduce all 21 facilities immediately.

Plan integration: these case IDs are recorded in the [expanded build catalogue](../build-plan/process-runtime-expansion.md). All remain planned, with conditional applicability preserved.
