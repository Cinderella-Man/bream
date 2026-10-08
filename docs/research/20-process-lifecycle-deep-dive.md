# Process lifecycle deep dive: spawn completion, relationship ownership, names, and exit continuation

**Execution follow-up:** [Report 29](29-pinned-reference-observations.md) now records targeted observations on the exact pinned build. The source-only status and hypotheses below describe the original review stage; the follow-up identifies which variants were executed and which remain unresolved.

Research snapshot: 2026-10-07. Read-only review of **Erlang/OTP 29.1.1**, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`, in `/home/bat/projects/bream/upstream/otp`. All upstream citations pin this revision. **No OTP build, test execution, failure injection, or latency measurement was performed.** Reading an upstream regression establishes what it checks; it does not record a passing run here.

Companion ownership boundaries: [signals, aliases and timers](/home/bat/projects/bream/docs/research/19-process-signals-timers-deep-dive.md), [inspection and control](/home/bat/projects/bream/docs/research/21-process-inspection-control-deep-dive.md), and [the signal-dispatch matrix](/home/bat/projects/bream/docs/research/24-process-signal-dispatch-matrix.md).

This extends [the lifecycle source review](/home/bat/projects/bream/docs/research/13-beam-lifecycle-source.md), [the edge audit](/home/bat/projects/bream/docs/research/16-lifecycle-edge-cases.md), and [the lifecycle plan](/home/bat/projects/bream/docs/build-plan/lifecycle.md). Existing LIFE-01–05 cases remain intact. New case IDs below are **research expansion candidates, planned and unimplemented**, not approval to add every reference feature to the first release.

Bream's accepted contract remains native Rust through ordinary Cargo, automatic suspension of admitted process computations, safe forced termination with runtime-owned cleanup, typed owned messages, growing selective mailboxes, one application/lifecycle envelope, and runtime-managed immutable bytes. Its DOWN guarantee requires stopped admitted application execution and removal from live lookup; final tracked reclamation may follow. BEAM's allowance for dirty native work to outlive logical death cannot weaken that requirement. Nothing below authorizes destruction of arbitrary Rust stacks, arbitrary user `Drop`, or unrestricted native calls.

## Coverage and method

**Traced** means the named entry, relevant branch, and consequent ownership/state transition were followed. **Sampled** means selected adjacent cases or tests were read without following all callees/configurations. **Unreviewed** means no coverage claim.

| Area | Depth | Boundary |
|---|---|---|
| Local spawn_opt/request, options/error classification, reply publication | Traced | All allocator failures and compiler argument-copy paths are not audited. |
| Distributed pending spawn, abandon, reply, parent exit | Traced at boundaries | Wire validation, old-node negotiation, transport failure permutations and codecs sampled/unreviewed. |
| Process monitor options, alias preservation, cancellation validity, tags | Traced | Alias delivery and priority insertion belong to the messaging audit; port/resource/node/time-offset internals sampled. |
| Repeated-link priority and relationship exit cleanup | Traced | Previous reports cover ordinary unlink/relink; not an exhaustive distributed unlink audit. |
| Local registration/lookup/unregister/death ordering | Traced for processes | Ports share registry; driver internals and distributed naming systems not audited. |
| References and node-incarnation rejection | Traced selected producers/consumers | External-term decoding and exhaustive wrap behavior unreviewed. |
| Exit phases and late signal dispatcher | Traced | ETS transfer, timers, GC, NIF destructors, inspection RPC handlers and encoding internals belong to other audits. |

## 1. Spawn entry points have different failure contracts

`erl_create_process` acquires the local parent's minor locks, inherits its group leader, and creates a reference when monitoring, requests or tracing need one. It validates MFA shape and argument-list length, selects initial scheduling flags, and obtains a process-table entry; table exhaustion reports SYSTEM_LIMIT. Arguments are sized/copied into child-owned storage. The reference is an operation/relationship identity, not the child PID. [Creation and capacity checks](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12436-L12546) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:12436)

| Entry/branch | Source path and result | Distinction |
|---|---|---|
| Local spawn/link/monitor | Common creation; link is a flag, monitor wrapper uses spawn options. | Child may finish before caller next runs, but initial atomic relationships already exist; report 13 traces front doors. |
| Local spawn_opt | BIF checks argument kinds, proper list and options; returns PID or `{Pid,Ref}`. Invalid option gives badarg with badopt cause. | Creation failure differs from successfully created entry function later raising. [spawn_opt BIF](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1027-L1073) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:1027) |
| Local spawn_request | Reference return plus success/error reply, according to policy. | Invalid member in proper options list gives asynchronous badopt reply; malformed list/type gives synchronous argument error through wrapper. [Request BIF](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1078-L1162) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:1078) [Public translation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L11256-L11263) · [local](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erlang.erl:11256) |
| Remote spawn_request | Caller installs pending monitor-like record and sends request; a later control signal completes it. | Request reference is not yet an active process monitor. [Pending dispatch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/dist.c#L6066-L6152) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/dist.c:6066) |
| Remote synchronous spawn_opt | Wrapper waits for request; on noconnection/notsup can spawn local `erts_internal:crasher/6` with options. | Returned PID need not be intended remote computation; local fallback preserves legacy result shape, then exits. [Remote wrapper](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L11040-L11077) · [local](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erlang.erl:11040) [Crasher](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erts_internal.erl#L1068-L1080) · [local](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erts_internal.erl:1068) |
| Remote spawn_monitor | Waits for request; noconnection creates local monitored crasher; other errors including notsup raise. | Do not flatten wrapper and request into identical result/failure channels. [Remote monitored wrapper](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L10859-L10912) · [local](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erlang.erl:10859) |

A local request performs creation before returning the reference; it is not a deferred launch queue. Creation installs child/parent links and monitor endpoints, queues an enabled success reply **as a child-originated message**, and only later schedules the child. The reply therefore precedes child signals in delivery order. Priority mailbox placement is a separate stage; this is not a blanket promise that a catch-all receive selects an ordinary reply before a later priority notification. [Priority versus signal order](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L376-L389) · [local](/home/bat/projects/bream/upstream/otp/system/doc/reference_manual/ref_man_processes.md:376). Success certifies creation, not entry-function existence or completion: undefined MFA can produce success then DOWN with `{undef,...}`. [Relationship and reply installation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12911-L13007) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:12911) [Schedule after setup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13115-L13135) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:13115) [Reply sender/ownership](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13146-L13193) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:13146) [Undefined entry regression](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L3834-L3854) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/process_SUITE.erl:3834)

### Options are parsed state

| Option family | Local parser behavior | Bream qualification |
|---|---|---|
| link / link options | Set flag; tuple delegates to link parser and records endpoint flags. | Atomic relationship is baseline; priority reception is expansion. |
| monitor / monitor options | Set flag; **plain monitor resets tag and monitor option flags**; tuple sets them from monitor parser. | Duplicate order can matter; do not treat every repeat as equivalent. |
| priority | Validate max/high/normal/low; set child's initial scheduling priority. | Separate scheduler priority from priority mailbox reception. |
| Heap/full-sweep/binary virtual heap | Validate values, floor/round heap sizes, compare max with min. | BEAM tuning knobs are not direct Rust layout requirements; detailed memory semantics elsewhere. |
| message_queue_data / async_dist | Set queue storage flags / child distribution flag. | Storage representation does not impose a bounded mailbox. |
| scheduler | Accept default zero or existing scheduler index; creation labels explicit binding unsupported in a source comment. | Reference-test aid is not automatically public Bream API. |
| Request reply policy | Set success/error suppression for yes/no/error_only/success_only. | Suppression does not cancel creation/relationships. |
| Request reply_tag | Arbitrary term replaces default tag. | Typed-envelope mapping unresolved; payload has cleanup cost. |

[Defaults and initial parser](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12220-L12320) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:12220) [Remaining parser/validation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12321-L12434) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:12321)

The parser records repeated settings in `multi_set`; duplicates are not uniformly rejected. Local parsing continues after an invalid member, so later reply options can affect the error reply. The distributed front end strips/retains certain options and has earlier bad-option exits. **No claim that every duplicate/invalid-option permutation behaves identically locally and remotely.** [Distributed option rewrite](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/dist.c#L5880-L6006) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/dist.c:5880)

## 2. A spawn request has its own lifetime

The request contract promises that a delivered success reply precedes child signals; requested monitoring/linking activates on successful completion. The request reference doubles as monitor reference, but demonitor cannot remove a pending request. Failure delivers no monitor DOWN and establishes no caller-side link. A remote noconnection failure means a child **may or may not** have been created; it is not rollback evidence. [Request contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L11270-L11406) · [local](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erlang.erl:11270)

| State/event | Branch and ownership consequence |
|---|---|
| Local request completes | Enabled reply already queued; no pending-spawn origin remains. Abandon returns false even before reply consumption. Upstream explicitly asserts it. [Local abandon checks](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L4033-L4048) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/process_SUITE.erl:4033) |
| Remote abandon, no link | Remove pending origin; remove/release distributed endpoint where still owned; later completion cannot deliver the operation reply/monitor result. |
| Remote abandon, link requested | Keep entry marked ABANDONED for later child cleanup; remove monitor-priority participation. This is not an active caller-side link. |
| Invalid/unrelated/completed abandon reference | Nonreference badarg; external/absent/already-abandoned/nonpending reference false. Wrong caller cannot find another caller's pending operation. [Abandon branches](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1166-L1207) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:1166) |
| Live caller gets error | Remove pending state; suppress message if abandoned/error replies disabled. |
| Live caller gets success | Install requested link; convert pending origin to active process monitor if requested, else remove; optionally materialize reply and transfer tag storage. |
| Abandoned linked request succeeds | Remove link ownership; send child abandoned only on original connection incarnation; suppress reply/remove pending state. Does not undo child effects. |
| No matching pending origin | Stale reply: release attached link/signal storage; no resurrection. [Stale reply](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L5690-L5725) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:5690) [Completion branches](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L5761-L5924) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:5761) |
| Connection breaks while pending | DOWN machinery makes a **four-element spawn error reply**, not ordinary monitor DOWN; honor suppression. [Disconnect conversion](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4838-L4910) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:4838) |
| Parent dies while pending | Transfer pending monitor into exit context. Linked case can suspend cleanup awaiting child result; eventually send parent reason or prior abandoned reason. No-link case releases. [Transfer to exit context](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13926-L13936) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:13926) [Pending child cleanup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13647-L13767) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:13647) |
| Reply reaches exiting parent | Resolve exit-context entry; demonitor successful child if needed; propagate parent/abandoned reason via link; release reply. Missing entry is possible after concurrent connection close. [Exiting-parent reply](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L5927-L6005) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:5927) |

The abandonment contract permits a child's ordinary messages to arrive afterward. “No future direct effect of the spawn operation” is narrower than “nothing associated with the child can happen.” An abandoned exit remains subject to eligible exit trapping; it is not direct forced kill. [Abandon contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L11451-L11492) · [local](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erlang.erl:11451)

Remote child initialization queues encoded arguments and a decode sentinel first while holding its mailbox lock, installs remote-side relationships, sends reply only on recorded connection incarnation, then schedules. Argument decoding can subsequently fail in the child. This is a launch protocol, not a recommendation to expose argument transport in Bream's application mailbox. [Distributed birth ordering](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13025-L13121) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:13025)

## 3. Monitor options expand identity and cleanup

The parser accepts priority, arbitrary tag, and three alias modes. Monitor alias addresses **the observer**, not the watched process. It uses a PID-reference carrying caller PID; ordinary monitor uses ordinary reference. Self-monitor is special: clear requested flags and install no monitor/alias entry. Duplicate tag/alias options overwrite earlier setting in this parser; invalid member/improper tail fails. [Options and local creation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L702-L798) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:702) [Alias contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L4665-L4696) · [local](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erlang.erl:4665)

| Mode/event | Reference lifetime |
|---|---|
| Ordinary monitor, demonitor | Remove origin; asynchronously cancel target endpoint. Later DOWN cannot materialize without valid origin. |
| explicit_unalias, demonitor | Replace origin with standalone alias record sharing reference; clear old alias ownership flags before release. Alias survives. |
| explicit_unalias, DOWN | Likewise preserve standalone alias while consuming monitor. |
| demonitor alias mode | Monitor removal, explicit or DOWN, deactivates alias. |
| reply_demonitor | First accepted alias reply or DOWN removes both; explicit unalias removes alias function while allowing monitor to remain. Delivery race belongs to messaging audit. |
| Already standalone alias | Demonitor treats as nonmonitor; must not deactivate preserved alias accidentally. |
| Pending spawn reference | Demonitor reports not removed; abandon is separate operation. |

[Demonitor alias branches](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L437-L502) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:437) [DOWN ownership](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6455-L6509) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6455) [Alias-mode semantics](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L4665-L4710) · [local](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erlang.erl:4665)

`down_on_alias_gh5310` addresses a real regression shape: demonitor replaces monitor with alias, target dies before target-side cancellation, compound DOWN arrives. It tests both immediate/compound reasons and separate scheduler assignments. [Late DOWN regression](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/monitor_SUITE.erl#L978-L1038) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/monitor_SUITE.erl:978)

### Tag and endpoint storage are runtime-owned

Local paired monitor data starts with two references. Immediate tag is stored directly; compound tag is copied into a monitor-owned heap fragment. DOWN conversion transfers/saves tag into its message; final destruction releases unused tag fragments and extended off-heap/distribution state. A pending spawn stores reply tag separately and must release it on abandoned/error destruction. Rust cannot retain a borrow into the observer's dropped/moving execution state merely because target holds the other endpoint. [Pair/tag allocation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.c#L856-L915) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_monitor_link.c:856) [Tag transfer](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L5023-L5044) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:5023) [Endpoint release](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.h#L1503-L1530) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_monitor_link.h:1503) [Final tag/monitor cleanup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.c#L1113-L1185) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_monitor_link.c:1113)

### Cancellation validates only cheap provenance

Demonitor distinguishes internal/external reference form, then searches **caller-owned monitor tree**. Missing internal reference is no-op; standalone alias/pending spawn is not removable as process monitor; found target endpoint is rejected. External refs from another node are rejected, while special external-ref/this-node path returns absent. Docs call a different process's reference erroneous but explicitly limit detection to cheap checks. An upstream test passes a reference created by the opposite local process and expects `true`: it does not remove the other process's origin. “Every wrong-owner reference raises” would overstate BEAM. [Reference checks](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L437-L468) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:437) [Incomplete provenance checking](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L2057-L2059) · [local](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erlang.erl:2057) [Wrong-owner/absent-ref tests](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/monitor_SUITE.erl#L179-L228) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/monitor_SUITE.erl:179)

Distributed demonitor separates local suppression from remote cleanup and uses existing distribution state; it need not reconnect just to cancel an obsolete observation. Target acknowledgment is not required for the observer-side future-notification suppression contract. [Remote demonitor](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L525-L592) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:525)

## 4. Link options, remote knowledge, and reference incarnation

Repeated active link is a single relationship, but caller endpoint options change: `link(P,[priority])` enables priority reception; `link(P)`/`link(P,[])` disables it. Peer setting stays independent. This is documented and explicitly tested locally/remotely. Unlink also removes local priority participation. “Idempotent link” cannot erase this branch. [Priority contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L3903-L3933) · [local](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erlang.erl:3903) [Flags before existing-link return](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L155-L183) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:155) [Enable/disable regression](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/signal_SUITE.erl#L1568-L1627) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/signal_SUITE.erl:1568)

| Boundary | Result and limit on inference |
|---|---|
| Local name/target missing; external PID from old incarnation of this node | DOWN noproc; source/tests explicitly cover old creation. |
| Remote monitoring cannot connect/connection breaks | DOWN noconnection; does not prove remote execution stopped. Cannot satisfy Bream's accepted local stopped-execution guarantee without a distinct distributed contract. |
| Connected remote target absent | Peer can answer noproc; distinct from inability to communicate. Full remote receiver protocol not audited here. |
| Remote request unsupported | notsup error, subject to legacy wrapper transformations above. |
| Cleanup after replaced connection | Original connection ID must match; same node name is insufficient to send old relationship cleanup over new connection. |

[Named/incarnation/remote monitor branches](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L804-L894) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:804) [Missing names and old creation with options](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/monitor_SUITE.erl#L1096-L1164) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/monitor_SUITE.erl:1096) [Spawn support check](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/dist.c#L6081-L6093) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/dist.c:6081) [Connection-bound demonitor](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13770-L13794) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:13770) [Deferred connection-bound link exit](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14012-L14049) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14012)

Reference identity is finite: schedulers advance individual 64-bit counters, nonscheduler threads share atomic counter. Thread identity/counter bits form reference numbers. PID references add marker and caller PID; alias delivery still requires live matching entry. Official limits document eventual reuse. Node incarnation and reference numeric identity solve different stale-state problems. [Reference encoding/counter](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_unique.h#L118-L177) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_bif_unique.h:118) [Ordinary/PID reference producers](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_bif_unique.c#L90-L168) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_bif_unique.c:90) [Finite reference space](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/efficiency_guide/system_limits.md#L92-L99) · [local](/home/bat/projects/bream/upstream/otp/system/doc/efficiency_guide/system_limits.md:92)

Earlier PID audit establishes full-PID validation and permitted finite reuse. This pass does **not** prove complete counter wrap safe across all encoded IDs, queues and relationships. Bream needs explicit runtime-instance/counter-exhaustion policy; internal retained work must remain bound to the actual record even if public IDs repeat.

## 5. Names have separate ownership and race boundaries

Local registry shares names across processes and ports. Registration takes an atom other than undefined and local live target; refuses already named target or occupied name. Registry/process locks protect both forward binding and target backpointer. To avoid lock-order deadlock, helpers may release caller locks before acquiring registry lock, then reacquire/revalidate. [Lock ordering](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/register.c#L52-L102) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/register.c:52) [Registration branches](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/register.c#L175-L264) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/register.c:175)

| Operation/race | Traced observation |
|---|---|
| whereis | Current PID/port or undefined; no reservation. ID-only helper does not test EXITING; locked live-record helper does. No continuing liveness guarantee. [Lookup variants](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/register.c#L277-L365) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/register.c:277) |
| Monitor name | Resolve current ID once; monitor that incarnation. Named DOWN reports `{Name,Node}`; later unregister/rebind does not retarget it. [Name snapshot](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L804-L808) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:804) [Named DOWN object](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4920-L4956) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:4920) |
| Another process unregisters | Permitted; no owner-only check. Clear target backpointer and erase binding. Invalid/absent name fails at BIF. [Registry BIFs](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L2310-L2350) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:2310) [Unregister mutation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/register.c#L507-L526) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/register.c:507) |
| Exit versus unregister/reuse | Exit unregisters its **current backpointer**, not saved name. If removed while caller lock released, no-op; old cleanup cannot erase replacement's binding. [Exit revalidation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/register.c#L450-L471) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/register.c:450) |
| Named target DOWN observed | Original registration released before notification. Third party may already have reused name; no promise that later whereis returns undefined. [Release before death notification](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14420-L14469) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14420) |

`monitor_SUITE:named_down` delays ordinary termination with priority work, then checks re-registration after DOWN. `register_SUITE:otp_8099` repeatedly races registration and killed predecessors. Useful future oracle seeds, not every registry schedule proven. [Named DOWN regression](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/monitor_SUITE.erl#L717-L760) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/monitor_SUITE.erl:717) [Registration stress](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/register_SUITE.erl#L48-L66) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/register_SUITE.erl:48)

## 6. Exit continues under runtime ownership

Distinct cut points are application execution stopped → live identity removed → relationship/control obligations → final physical release. Not every resource is deferred: registered names release before observers are notified. `erts_do_exit_process` takes all locks for the transition, marks exiting, cancels process timer, and rejects outstanding code-permission ownership. A system-process exit is a separate runtime-fatal branch, outside ordinary Bream process semantics. [Exit entry](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14170-L14218) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14170)

| Exit phase | Work and continuation boundary |
|---|---|
| TIMERS | Cancel BIF timers with resumable cursor; destroy paused BIF timers; abort scheduler-online wait. |
| BLCKD_MSHED / BLCKD_NMSHED | Release multi-scheduling blocks owned by process. |
| USING_DB | Perform ETS process-exit work, which may yield. |
| CLEAN_SYS_TASKS | Protect literals when reason/native access requires; enable GC; drain system tasks with budget. |
| FREE | Release dynamic-driver ownership, unregister name, preserve relationship roots, remove table identity, set FREE. |
| CLEAN_SYS_TASKS_AFTER | Drain tasks which could arrive until FREE; handle distribution-controller exit; disable GC to keep reason stable. |
| Capture transport/signal state | Retain suspended fragmented-send continuation; deinstall incoming buffers and fetch remaining signals. |
| LINKS | Yielding link traversal; unlinking endpoints do not propagate ordinary link exit. |
| MONITORS | Outgoing watches cancel, incoming watches notify, pending spawns transfer to special exit state. |
| LT_MONITORS | Yielding local incoming-monitor list traversal. |
| HANDLE_PROC_SIG | Budgeted non-message signal drain with exiting semantics. |
| DIST_SEND | Finish transport continuation; too-large encoding may terminate that connection. |
| DIST_LINKS / DIST_MONITORS | Encode/send deferred compound reasons through yielding traversal and resumable transport. |
| DIST_PEND_SPAWN_MONITORS | Resolve child-creation obligations; linked unresolved request can suspend cleanup waiting for completion/connection close. |
| DONE | No more yielding allowed. Clear active flags; protect queued proxy references; schedule thread-progress release; delay deletion if dirty scheduler still owns record. |

[Timers/blocks/ETS/system tasks](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14293-L14402) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14293) [Registry/table death and transport capture](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14406-L14543) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14406) [Relationships/signals/distribution](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14545-L14698) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14545) [Final release and yield](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14700-L14832) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14700)

Two system-task passes are intentional: signals can create priority-elevation tasks until FREE. GC is enabled during early cleanup, then disabled while reason is retained across notifications. Yield stores phase/cursor and resumes `beam_continue_exit`; application code stays stopped. **A yielding outer traversal does not prove each callback or destructor bounded.** Final deletion and foreign-resource callbacks require their own resource/latency audit. This is the important difference between a scheduled cleanup protocol and a promise of constant-time kill/reclamation.

### Relationship dispatch is type-specific

Incoming process/suspend monitors send DOWN; incoming NIF-resource monitors invoke the NIF-monitor callback path; port monitors invoke port logic. Outgoing process/suspend watches send demonitor; time-offset/node/nodes/port kinds have specialized cancellation. Standalone alias origin releases. Distributed incoming monitors with immediate reasons can send directly; compound reasons move to deferred tree for encoding. Remote outgoing cancellation is connection-incarnation-bound; pending spawn has parent-exit protocol above. These are not all identical one-shot process monitors. [Monitor exit dispatch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13798-L13977) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:13798)

Local links send committed reason unless unlinking. Distributed links separate immediate/deferred compound reasons and verify connection identity. Receiving-side active-link/trap/direct-kill rules remain as in report 13; cleanup does not turn every literal kill reason into an untrappable direct kill. [Link exit dispatch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14068-L14165) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14068)

### Late control signals still require answers

| Signal during exit drain | Action |
|---|---|
| Direct exit / linked exit / DOWN | Release signal/relationship storage; do not rewrite committed reason. |
| Alias delivery / persistent monitor / NIF-select | Discard owned message storage. |
| New monitor | Reply noproc through monitor-exit handler. |
| Demonitor | Release cancellation representation. |
| New link | Send linked noproc. |
| Unlink | Acknowledge, with local/remote/port variants. |
| Unlink acknowledgment | Destroy operation record; no relationship recreation. |
| Is-alive | Reply false. |
| Process-info / group-leader / suspend / RPC | Dispatch designated handler in exit state; full individual handler contracts are outside this report. |
| Remote spawn reply | Resolve pending child/link cleanup against exit context. |
| Queue adjustment / flush / tracing / receive marker | Complete internal cleanup/bookkeeping and marker release. |

Dispatcher budgets work, preserves remainder on yield, and clears signal flag once drained. It visits **non-message** signals; application mailbox storage has a different release path. Blanket dropping the dead process queue would lose acknowledgments, child obligations and inspection replies. [Exiting signal dispatcher](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7259-L7473) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:7259)

## 7. Two observations still need exact-version execution

### Synthetic five-tuple and demonitor flush

Pinned docs describe removing one `{_,Ref,_,_,_}` and give unconditional zero-timeout receive equivalence. Pinned code for ordinary active one-shot monitor removes origin and returns true with multi=false; demonitor_2 returns **without calling the helper**. In the absent/relevant multi/yield branches, helper really matches any five-tuple with Ref in position two. [Documented equivalence](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L2076-L2089) · [local](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erlang.erl:2076) [Active removal](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L437-L502) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:437) [Conditional helper](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L613-L662) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:613) [Actual receive](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erts_internal.erl#L515-L534) · [local](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erts_internal.erl:515)

**Source-predicted edge:** create ordinary active monitor on blocked live child, self-send `{anything,Ref,a,b,c}`, then demonitor with `[flush]`: synthetic tuple appears to remain. That differs from literal broad equivalence. This has not been executed and is **not a confirmed upstream bug**. Probe active, absent, already-DOWN, alias, custom-tag, and info+flush separately. Ordinary genuine-monitor suppression is unaffected: a genuine one-shot DOWN and removable origin do not coexist. Bream's typed envelope has not settled whether application code can forge lifecycle variants or how broad flush matching maps into it. Do not silently forbid/allow constructors or pick matching rules.

### Combined EXIT and DOWN

For an ordinary established local link plus monitor, no priority reception, and trapping observer, source sends link exit in LINKS before local DOWN in LT_MONITORS, both using dying PID as sender. This supports **source prediction** EXIT-before-DOWN in the ordinary mailbox; it is neither an executed result nor a universal promise across mixed priority, distribution, cancellation or unrelated targets. [Traversal order](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14545-L14584) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14545) [Link sender](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14076-L14082) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14076) [DOWN sender](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13806-L13814) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:13806)

The request test selectively receives DOWN before EXIT, so it cannot establish arrival order: receive may skip earlier EXIT. Oracle must collect both with catch-all receive sequence, separately checking reply-before-child-signals. Priority on only one relationship can change mailbox placement while preserving signal arrival order; see the messaging audit. [Selective test limitation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L3545-L3564) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/process_SUITE.erl:3545)

## 8. Selected upstream tests and their limits

| Test/read range | Evidence and limit |
|---|---|
| process_SUITE request tests | Result shapes and atomic relationships; nested selective receives are not arrival-order oracle. [Request tests](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L3530-L3604) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/process_SUITE.erl:3530) |
| spawn_request_abandon_bif | Local abandon false; remote stress includes winning abandonment and already-completed requests. Not deterministic scheduler proof. [Abandon tests](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L4033-L4145) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/process_SUITE.erl:4033) |
| Reply policy tests | Enabled/suppressed success/error locally/remotely, invalid options. Not every duplicate order. [Reply-policy tests](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/process_SUITE.erl#L4364-L4426) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/process_SUITE.erl:4364) |
| monitor_tag_storage | Tagged DOWN on heap, overflow heap fragment, off-heap queue, GC. No mandatory Rust allocation strategy. [Tag storage](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/monitor_SUITE.erl#L874-L941) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/monitor_SUITE.erl:874) |
| unexpected_alias_at_demonitor_gh5310 | reply_demonitor alias must not accidentally remain active after cancellation. [Alias removal](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/monitor_SUITE.erl#L952-L975) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/monitor_SUITE.erl:952) |
| down_on_alias_gh5310 | Preserved explicit alias must not accept late DOWN after cancellation; immediate/compound reasons. [Late DOWN](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/monitor_SUITE.erl#L978-L1038) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/monitor_SUITE.erl:978) |
| monitor_3_noproc_gh6185 | Missing names and old creation with tags/explicit alias. Observer-exit companion explicitly seeks crashes/asserts/leaks, not result validation. [Missing target/options](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/monitor_SUITE.erl#L1041-L1164) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/monitor_SUITE.erl:1041) [Observer-exit stress](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/monitor_SUITE.erl#L1182-L1259) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/monitor_SUITE.erl:1182) |
| named_down / otp_8099 | Name released before DOWN and registration/death stress; no vacancy guarantee against third parties. [Named DOWN](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/monitor_SUITE.erl#L717-L760) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/monitor_SUITE.erl:717) [Registration stress](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/register_SUITE.erl#L48-L66) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/register_SUITE.erl:48) |
| priority_messages_link_enable_disable | Local/remote endpoint toggles, unlink, normal exit, disconnect, spawn forms. [Priority lifetime](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/signal_SUITE.erl#L1568-L1627) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/signal_SUITE.erl:1568) |
| demon_e_1 / demon_1 | Detectable invalid refs versus unrecognized local ref no-op; incomplete creator validation. [Demonitor validity](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/monitor_SUITE.erl#L179-L218) · [local](/home/bat/projects/bream/upstream/otp/erts/emulator/test/monitor_SUITE.erl:179) |

## 9. New concrete plan candidates

Every row is **planned/unimplemented**. Reference behavior should be preserved if the feature is admitted; oracle labels remain unresolved before claiming compatibility. Existing LIFE-01–05 stay baseline. Distribution, aliases, names and priority are expansion inventory, not automatic initial scope. A test may use upstream facilities Bream will not implement.

### LIFE-06 — Expanded spawn/request/options (17 cases)

| ID | Trigger → outcome or unresolved oracle | Implementation/test obligation |
|---|---|---|
| LIFE-06.C01 | Wrong argument kind or improper args/options list → synchronous argument error. | Enumerate overload/type validation apart from async errors; map Rust compile-time exclusions explicitly. |
| LIFE-06.C02 | Proper list with invalid option → request ref plus enabled error/badopt reply; spawn_opt raises. | Keep front-door versus operation failure distinct. |
| LIFE-06.C03 | Invalid option plus reply/tag options in both orders → local/remote precedence oracle. | Do not normalize away parser-order effects. |
| LIFE-06.C04 | Each reply policy × success/error → exactly enabled reply class. | yes/no/success-only/error-only; suppressing success still creates child. |
| LIFE-06.C05 | Compound reply tag × success/failure/abandon/parent death → valid payload or one safe release. | Pending tag ownership; typed tag API unresolved. |
| LIFE-06.C06 | Atomic link+monitor request, immediately exiting/sending child → enabled success reply delivered before child signals. | With default reception priority, collect complete mailbox sequence; separately test signal delivery versus priority placement. |
| LIFE-06.C07 | Local completed request, reply unread → abandon false. | Unread reply is not pending operation. |
| LIFE-06.C08 | Remote completion races abandon → one effective terminal state; nonref errors, unrelated/finished refs false. | Gates for pending/received/abandoned states and owner-local lookup. |
| LIFE-06.C09 | Abandon linked request wins, then child created on still-usable original connection → child gets abandoned, parent no operation reply/relationship. | Child may trap reason or send other messages; no rollback claim. |
| LIFE-06.C10 | Parent dies before remote linked request completes → child gets parent reason or prior abandoned reason. | Retain pending operation and original connection in cleanup continuation. |
| LIFE-06.C11 | Success reply suppressed but link/monitor requested → relationships still activate on success. | Reply presence cannot be sole activation event. |
| LIFE-06.C12 | Created child has missing MFA/argument decode failure → success then child failure. | Rust entry admission differs; define applicable analogue, not fictitious launch failure. |
| LIFE-06.C13 | Disconnect during remote request → noconnection; child may exist. | Never infer not-created or remote-stopped universally. |
| LIFE-06.C14 | Stale reply/old connection → release attached ownership; no resurrection/new-connection effect. | Inject stale control representations in later internal tests. |
| LIFE-06.C15 | Remote synchronous wrapper failure → preserve documented PID/monitor/exception shape. | Later compatibility layer needs crasher fallback; not local-first prerequisite. |
| LIFE-06.C16 | Duplicate monitor/link option forms → versioned order oracle. | Plain monitor after tagged/aliased form; local versus distributed rewrite. |
| LIFE-06.C17 | Initial options/capacity failure → settings before first run; no live partial child/relation on failure. | Couple EXEC accounting; allocator fault-injection proof outstanding. |

### LIFE-07 — Relationship options, references and exit continuations (18 cases)

| ID | Trigger → outcome or unresolved oracle | Implementation/test obligation |
|---|---|---|
| LIFE-07.C01 | Immediate/compound custom DOWN tag, normal/missing target → valid payload through receipt/discard. | Ownership across allocation, kill, cancel and queued notification cleanup. |
| LIFE-07.C02 | explicit_unalias monitor cancelled → standalone alias survives, observation ends. | Replacement alias must not retain monitor behavior. |
| LIFE-07.C03 | explicit_unalias gets DOWN, including noproc → alias survives until unalias/owner death. | Preserve alias while destroying monitor/tag endpoints. |
| LIFE-07.C04 | demonitor/reply_demonitor modes × cancel/DOWN/first reply/unalias → mode contract. | Couple alias delivery; no duplicate release. |
| LIFE-07.C05 | Late compound DOWN after monitor-to-alias replacement → no notification resurrection. | GH5310 schedule, immediate and compound reasons. |
| LIFE-07.C06 | Repeated link priority toggle → caller endpoint only, one relation. | Distinguish scheduling/reception priorities. |
| LIFE-07.C07 | Last priority relationship exits/cancels/abandons → delete bookkeeping once. | Surviving priority alias separate from removed monitor. |
| LIFE-07.C08 | Self-monitor with alias/tag/priority → ref, no relationship. | Exact option-effect oracle; no phantom alias. |
| LIFE-07.C09 | Wrong-owner local/remote ref to demonitor → documented cheap checking. | Never cancel another owner's relationship; Rust error/provenance surface unresolved. |
| LIFE-07.C10 | Pending spawn ref or standalone alias to demonitor → no process-monitor removal. | Distinct reference kinds/states. |
| LIFE-07.C11 | Old creation versus remote disconnect → noproc versus noconnection. | Never infer remote stop from lost connection. |
| LIFE-07.C12 | Full ref-counter/runtime-instance reuse → explicit identity policy. | Tiny-counter model, serialization and retained-work tests; full wrap proof absent. |
| LIFE-07.C13 | Exit owning timer/block/ETS-equivalent → required pre-notification release, resumable phases. | Only admitted kinds apply; callbacks not presumed bounded. |
| LIFE-07.C14 | Compound reason across cleanup yields/GC-equivalent work → stable committed reason. | Runtime-owned reason outlives all notification continuations. |
| LIFE-07.C15 | Late monitor/link/unlink/info/RPC → correct response/release in exit state. | Expand LIFE-05.C10 dispatcher variants; no application resurrection. |
| LIFE-07.C16 | Kill during fragmented transport/pending linked spawn → finish runtime protocol obligations. | Later distribution cleanup; retained refs tracked after DOWN. |
| LIFE-07.C17 | Synthetic versus genuine tuples, active/absent flush/info → exact oracle and typed-envelope mapping. | Docs/source discrepancy open; do not silently choose constructors/matching. |
| LIFE-07.C18 | Combined EXIT/DOWN × local/remote, immediate/compound, mixed priority → reference order/allowed set. | Refine existing case with catch-all traces and sender/relationship metadata. |

### NAME-01 — Local registration lifetime (8 cases)

| ID | Trigger → required outcome | Implementation/test obligation |
|---|---|---|
| NAME-01.C01 | Unused valid name/local live unnamed target → one binding/backpointer. | Typed namespace/protocol mapping unresolved; no partial mutation. |
| NAME-01.C02 | Occupied name, named/dead/invalid target, invalid name → failure without changed binding. | Competing registration oracle; shared port namespace only if ports admitted. |
| NAME-01.C03 | whereis then death/rebind → snapshot, no reservation. | Subsequent PID action independently resolves; no service-handle retargeting. |
| NAME-01.C04 | Monitor name then unregister/rebind → original lifetime observed, named object reported. | Target incarnation independent of current registry. |
| NAME-01.C05 | Another process unregisters → allowed; invalid/absent fails. | No owner-only rule misrepresented as BEAM default. |
| NAME-01.C06 | Exit races unregister/reuse → old cleanup cannot remove replacement binding. | Revalidate current backpointer under lifetime synchronization. |
| NAME-01.C07 | Named target DOWN → old binding released; third-party replacement allowed. | Named-down regression with/without competing registrar. |
| NAME-01.C08 | Registry contention plus kill → no deadlock, dangling record or stale binding. | Lock/lifetime strategy and test proof outstanding. |

**43 expansion/refinement cases:** LIFE-06 17, LIFE-07 18, NAME-01 8. Dependencies are shared specifications with admission, messaging, scheduler and resources; this inventory does not prescribe a cyclic implementation order.

## Remaining audit gaps

Unreviewed or sampled: all spawn overload/error permutations; allocator rollback with fault injection; complete counter/PID wrap; reconnection × request/abandon/parent-death interleavings; persistent time-offset/node monitors; ports/NIF-resource callback implementations; registry/port interleavings; full inspection/RPC exit responses; tracing/sequence-trace lifetime; hot-code/literal protection beyond exit entry; final destructor/allocator latency. Parallel audits address several areas, but unfinished work cannot count as this report's traced coverage.

Two exact-version oracles remain explicitly open: synthetic flush equivalence and complete EXIT/DOWN ordering across stated variants. Duplicate-option permutations and self-monitor option effects also require execution before making public compatibility promises. Source tracing establishes concrete branches and ownership transitions; it does not establish behavior in every case or certify a Rust runtime's safety.

Plan integration: these case IDs are recorded in the [expanded build catalogue](../build-plan/process-runtime-expansion.md). All remain planned, with conditional applicability preserved.
