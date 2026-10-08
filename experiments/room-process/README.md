# Bounded room/session program from checked processes

**Passed for the fixed two-session single-worker profile, 2026-10-08.** This ninth bounded milestone integrates the [atomic relationship-spawn profile](../spawn-process/README.md) into a raw room/session application. The runtime and checked compiler retain that profile, apart from experiment-specific crate bindings. The new work is a separate consuming application's protocol and correctness controls. It begins G5 integration; it does not complete the broader lifecycle or application gates. [Correctness cohort](../../evidence/room-process/runs/20261008-room-01/results.json), [audit](../../evidence/room-process/validation.json), [milestone tracker](../../docs/milestones.md).

**No speed tests or performance comparisons may run on this machine.** Source inspection, builds and bounded correctness controls are the available evidence. Logical events and injected ticks establish semantic order only. Watchdog timeouts detect hangs; their elapsed values are not performance evidence. Performance qualification requires a separately authorized suitable environment. [Persistent instructions](../../AGENTS.md).

## Topology and ownership

The [checked application](consumer/src/room.rs) contains a room, exactly two session children and a collector. The room uses `spawn_monitor` to create each session with an owned startup buffer. It retains a checked `Member` record containing the child's full process identity, initial monitor reference and membership/lifecycle flags. Each session monitors the room in the opposite direction and returns when the room's genuine Down arrives. These are separate one-way monitors, not links. Session failure therefore removes one member without implicitly killing the room or its other session. The inherited link controls remain the separate evidence for linked failure behavior.

The host supplies the collector address as the room's input target and the two startup buffers as setup messages with tags `1` and `2`. The collector is itself a checked process. An independent checked process runs an infinite CPU loop while the application conversation proceeds; another process observes its eventual forced stop. This checks logical peer progress and external stop on one worker, without measuring their speed.

Application messages use the inherited envelope: a numeric tag plus one moved `Words` buffer. Requests, room commands, session replies and collector records transfer ownership of that buffer. Responses reuse the request allocation instead of building another payload. This is a structural ownership property, not a measured performance improvement. A receipt retained by the host tracks the original allocation through transfer and final release.

The two explicit member records are a bounded use of admitted state, not a general membership collection. Numeric session slots and tags identify fixture roles; they do not authenticate a sender. Genuine Down events are matched against both the saved complete PID and monitor reference. The application cannot manufacture these runtime notifications. There is no generic PID-bearing application payload, immutable shared buffer or broadcast operation in this slice.

## Explicit fixture protocol

Each ordinary host request has eleven words. The host initially supplies the selected session slot in word `0`, a correlation sequence in word `9`, and a value in word `10`; unused fields begin at zero. A reply record replaces these with the following fields:

| Word | Meaning |
|---|---|
| `0` | Result code |
| `1` | Current membership mask |
| `2` | Accepted joins |
| `3` | Duplicate join requests |
| `4` | Delivered publish replies |
| `5` | Graceful leaves |
| `6` | Failed sessions |
| `7` | Retired sessions |
| `8` | Protocol errors |
| `9` | Request correlation sequence |
| `10` | Request or transformed payload value |

The room forwards response records to the collector with tag `300`.

| Host tag | Request | Observable policy |
|---|---|---|
| `101` | Join/duplicate join | A known existing member follows the session handshake; a duplicate increments the duplicate counter without installing another monitor. |
| `102` | Publish to one selected session | The session adds `slot * 100` to the value and reports it to the room. Sending to an absent member produces an application result instead. |
| `103` | Leave | The selected session reports its graceful departure and returns. A repeated leave is idempotent. |
| `104` | Fail one selected session | The session first sends a fault report, then takes the admitted deliberate failure path. Its Down drives retirement. |
| `105` | Query | Return the room's current membership and counters. |
| `106` | Selective wait | Wait for application tag `909` using a five-tick injected-clock timeout. Unmatched requests remain queued for later handling. |
| `107` | Stop the room | Complete the orderly scenario after both session children have retired. |

Initial sessions send join tags `11` and `12`; the room acknowledges with `100`, and sessions confirm with `21` and `22`. The initial sequence-zero handshake emits no collector record. Subsequent room-to-session commands use tags `201` for join, `202` for publish, `203` for leave and `204` for deliberate failure. Sessions return publish replies as `51`/`52`, graceful-leave reports as `61`/`62`, and fault reports as `71`/`72`.

| Result code | Meaning |
|---|---|
| `1` | Membership query |
| `2` | Duplicate join |
| `3` | Repeated leave |
| `4` | Selected member is unavailable, including publish to an absent member |
| `5` | Unknown session slot |
| `6` | Selective receive timed out |
| `7` | Publish reply delivered |
| `8` | Session fault report |
| `9` | Orderly room stop |
| `10` | Graceful leave report |
| `11` | Tag `909` satisfied the selective wait; not exercised by the new conversation |
| `12` | Stop requested before both children retired; not exercised by the new conversation |

Join and leave idempotence, result codes and the fixed two-session topology are reversible application policies. They are not claimed as BEAM semantics or a supervision policy. The room checks that the expected session report precedes its Down, updates the matching member only once, and records retirement separately from an application leave report. An already queued notification is not assumed to disappear merely because membership changed.

A session records diagnostic result `999` for an unknown command, a Down that does not match its room PID and monitor reference, or an unexpected Exit, then continues receiving. Those invalid inputs do not count as a successful session termination. Normal session return occurs after an explicit leave or the matching room Down; the deliberate failure command remains a separate abnormal path. This matters to the forced-room-death control, which sends neither leave nor failure: the remaining live-process count must fall only after both sessions accept their room notification.

## Correctness observations

The host keeps an explicit expected transcript of fifteen records. The checked collector records the number of received records and a cumulative wrapping base-131 digest over every word of every eleven-word record. After each protocol barrier, the host compares the observed count and digest with the expected prefix. It retains fourteen actual collector snapshots as `observed_checkpoints`, each containing `[record_count, digest, sequence]`; the timeout and following publish share one barrier. Final `observed_records` and `observed_digest` come from the collector's final snapshot. Saved transcript rows are labeled **expected**, rather than presented as raw observed payloads. A count and digest check is a bounded oracle; it is not a proof that collisions are impossible or an exhaustive protocol-interleaving test.

The controls exercise startup, a duplicate join for the first member, routing and transformation of an owned payload, ordinary and repeated leave, an unknown slot, a session's report-before-failure ordering, exact monitor-based membership removal and continued communication with the surviving session. Selective receive uses injected clock advancement, then verifies that an unmatched publish survives and its reply follows the timeout report. The independent hog must make logical progress while the room and collector complete their conversation; the host then kills it and its observer verifies genuine Down.

Separate teardown cases force the room to stop while it is held in a finite receive with nineteen queued requests, and shut down the host runtime during active communication. They account for all tracked input and message receipts, ensure session room monitors end the children after forced room death, distinguish stop from cleanup completion, and check that runtime relationships and timers settle. Input checks require exactly one release of the room's eight bytes and the independent computation's sixty-four bytes, as well as destruction of the collector's and observer's zero-capacity inputs. Transferred message and startup buffers are checked at their original eighty-eight-byte receipts. A shutdown observer may finish with the hog's forced-stop reason or itself be stopped by shutdown; successful observer completion must report reason `2`. These controls use watchdogs only as failure containment.

The [retained correctness evidence](../../evidence/room-process/README.md) passes **317 expected command outcomes** and preserves raw outcomes, source identities and the expected transcript alongside actual digest/count/sequence checkpoints. Successful inherited unit and boundary controls establish only their already declared restricted profile; they do not substitute for the new application's transcript and ownership assertions.

The passing cohort runs all three application scenarios at allowances `1`, `4` and `32` in debug and release: six public room configurations and eighteen scenario executions. It also retains the inherited eighty-three unit tests per profile, 124 expected compilation rejections per profile and seven positive boundary examples per profile. The audit verifies 2,401 retained hash records, 156 current inputs and all fifteen inherited runtime/compiler files after crate-name normalization.

## Pinned BEAM obligations

The inspected local OTP source is pinned to `ad05823719d77c8faee87348ea39513d4e2f99c5`. This source review did not execute BEAM.

| Obligation | Reference |
|---|---|
| A monitor is one-way; Down identifies its reference, target and reason. Repeated monitor calls are independent. | [Monitor contract](../../upstream/otp/system/doc/reference_manual/ref_man_processes.md:707) |
| Initial monitored spawn installs the relationship before scheduling the child. | [Monitor insertion](../../upstream/otp/erts/emulator/beam/erl_process.c:12932), [child scheduling](../../upstream/otp/erts/emulator/beam/erl_process.c:13121) |
| A signal becomes Down only while its origin monitor remains active. | [Down conversion](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6374) |
| Removing a monitor prevents future Down insertion but preserves an already queued notification. | [Demonitor contract](../../upstream/otp/erts/preloaded/src/erlang.erl:2030), [origin removal](../../upstream/otp/erts/emulator/beam/bif.c:488) |
| Signals from the same sender to the same destination preserve order; ordinary messages enter at the queue's end. | [Signal ordering](../../upstream/otp/system/doc/reference_manual/ref_man_processes.md:507), [ordinary queue ordering](../../upstream/otp/system/doc/reference_manual/ref_man_processes.md:349) |
| Exit cleanup sends monitor notifications with the exiting process as sender. | [Monitor exit handling](../../upstream/otp/erts/emulator/beam/erl_process.c:13811) |
| Selective receive chooses the first matching message and leaves other messages unchanged. | [Receive contract](../../upstream/otp/system/doc/reference_manual/expressions.md:680) |
| Timeout resets the saved scan position; receive scanning charges work and can yield. | [Unmatched-message scan and timeout](../../upstream/otp/erts/emulator/beam/emu/msg_instrs.tab:249), [receive yield check](../../upstream/otp/erts/emulator/beam/emu/msg_instrs.tab:65) |
| Fatal signal handling does not require the process to call application receive. | [Signal handling](../../upstream/otp/system/doc/reference_manual/ref_man_processes.md:282) |
| Generated execution checks reduction exhaustion and switches processes. | [Generated yield check](../../upstream/otp/erts/emulator/beam/jit/x86/instr_common.cpp:3236), [switch fragment](../../upstream/otp/erts/emulator/beam/jit/x86/instr_common.cpp:3206) |

Report-before-Down follows from same-sender ordering and the exit notification's sender identity. It is a mailbox-order statement for ordinary local signals, not a requirement that every selective receive consume messages in that order. The fixture's use of the shared event stream makes this order observable. Different sessions' messages have no global BEAM ordering guarantee; explicit protocol barriers define the fixture's expected conversation.

The inherited [BEAM-default decision](../../docs/adr/0010-beam-observable-semantics-by-default.md), [primitive-first scope](../../docs/adr/0004-beam-primitives-first.md) and [stop-before-notification decision](../../docs/adr/0009-death-and-reclamation-observation.md) supply the semantic direction. Rust-specific opportunities here are explicit allocation transfer across the whole request/reply path, sealed runtime notifications, and checked application state assembled without introducing a server framework. No speed claim follows from these choices.

## Scope still open

This is a restricted part of [APP-01](../../docs/build-plan/admission-and-conformance.md:138). Full G5 still requires publish-to-zero/one/many fanout, retained immutable shared payloads across publisher/receiver death, a session that itself hogs computation, larger membership or parser work with partial cancellation, and broader join/fanout/death interleavings. The [accepted immutable-sharing model](../../docs/adr/0008-immutable-buffer-sharing.md) is unchanged; moved `Words` do not qualify it.

The experiment also leaves general dynamic membership, generic typed PID-bearing protocols, arbitrary exits and reasons, real-time timers, multiple workers and full BEAM conformance open. No restart policy, networking, registry, supervisor or automatic mailbox migration is introduced.

The next bounded milestone is runtime-managed immutable payload sharing and bounded broadcast to zero, one and two recipients, with surviving-reader and exactly-once final-release checks.
