# Local monitors and selectable Down notifications

**Passed for the restricted local single-worker profile, 2026-10-08.** The [final cohort](../../evidence/monitor-process/runs/20261008-monitor-02/results.json) and [read-only audit](../../evidence/monitor-process/validation.json) record 221 expected command outcomes: 51 unit tests per compiler profile, six monitor controls, inherited mailbox/timeout controls, 178 compilation rejections and ten positive shadowing executions. [Evidence and reproduction](../../evidence/monitor-process/README.md).

The sixth bounded milestone extends [receive timeouts](../receive-timeout/README.md) with independent local monitors and genuine `Down` notifications in the same ordered mailbox as application messages. It covers monitoring live, absent, self and reused identities; creation versus death; cancellation before or after notification conversion; and observer cleanup. This remains a restricted single-worker native Rust experiment with an injected logical clock. [Milestone tracker](../../docs/milestones.md).

**No speed tests or performance comparisons may run on this machine.** Logical event counters and injected clock ticks establish semantic order, not elapsed-time results. Watchdogs detect hangs only. Performance qualification requires a separately authorized suitable environment. [Persistent instructions](../../AGENTS.md).

## Checked application boundary

The separate consumer enters through the [checked grammar](macros/src/structured.rs) and [runtime context](runtime/src/lib.rs). These are experimental authored operations, not a finalized library interface.

| Authored operation or pattern | Meaning |
|---|---|
| `monitor(pid)` | Return a fresh opaque `MonitorRef` for this observer and exact target incarnation. |
| `demonitor(reference)` | Cancel future genuine Down conversion; leave an already queued Down available. |
| `demonitor_flush(reference)` | Cancel, then selectively remove the corresponding queued Down if present. Scanning can yield automatically. |
| `receive_event()` / `receive_event_for(ticks)` | Select the oldest event across application messages and Down notifications; the finite form can time out. |
| `receive_down(reference)` / `receive_down_for(reference, ticks)` | Select the earliest Down with that exact monitor reference, retaining other events. |
| `Event::Message(message)` / `Event::Down(down)` | Inspect an owning result from the shared mailbox. |
| `EventResult::Event(event)` / `EventResult::Timeout` | Inspect a finite receive across both event kinds. |
| `DownResult::Down(down)` / `DownResult::Timeout` | Inspect a finite reference-selective receive. |
| `down_ref(down)` / `down_target(down)` / `down_reason(down)` | Read the observation identity, exact target identity and restricted reason code. |
| `receive(tag)` / `receive_any()` and their `_for` forms | Existing application-message selectors over the shared queue. `receive_any` skips Down; it does not mean any runtime event. |

`MonitorRef` and `Down` have private runtime fields. A copied reference owns no process storage, and dropping it does not cancel the registration. Application construction or reinjection of lifecycle notifications, arbitrary predicates, external futures, custom destructors and general callbacks remain outside this profile. This restriction does not settle the future public mapping of synthetic BEAM notification look-alikes.

Reason codes are deliberately small prototype values: **1** normal completion, **2** killed, **3** process failure, **4** absent incarnation (`noproc`). They are not arbitrary Erlang exit terms. Separate monitor calls for one target create separate references and notifications. Self-monitoring returns a fresh reference without installing a relationship. Monitoring an absent or obsolete incarnation queues a later `noproc` conversion rather than recovering its historical exit reason.

Trusted host controls expose `RuntimeClient::monitor(observer, target)`, `demonitor_info(observer, reference)` and diagnostic active-registration counts. The observer must be live at creation; a foreign-runtime or stale target is absent in this local profile. The info boolean describes origin removal, not target liveness or whether a mailbox item was flushed. Cross-runtime identity routing is not distributed monitoring.

Finite receives inherit the injected-clock rules: logical ticks in `0..=u32::MAX`, scan before waiting, zero-timeout scanning, one timer per wait, eager cancellation on selection, and exact receive-generation checks. A committed expiry wins at worker reentry. There is no real-time clock driver or concurrent in-poll timer callback qualification.

## Cancellation, ordering and ownership

[Monitor storage](runtime/src/monitor.rs), [mailbox selection](runtime/src/mailbox.rs) and [retirement](runtime/src/lifecycle.rs) enforce separate transitions:

1. Creation and target retirement serialize through the process registry. A relationship established before retirement receives the committed reason; creation after live identity removal observes absence. Full runtime, slot and generation identity prevents an old watch from following a reused slot.
2. The observer's active registration is permission to materialize Down. Cancellation and conversion serialize under the monitor-table lock, which remains held through mailbox insertion. Cancellation can suppress an internal pending Down even after target death. Once conversion has happened, ordinary cancellation leaves the mailbox item intact.
3. Explicit flush searches the same queue, retaining unrelated application messages and other monitor references in order. Saved nonmatches return before the incoming suffix. Scan work consumes the process allowance; flush does not install a permanent reference blacklist.
4. Stop occurs outside application polling, closes the mailbox and cancels its timer, removes live identity, records the terminal state, then detaches relationship indices. Down conversion follows irreversible application stop and identity removal. It need not wait for final tracked reclamation; receiving Down is not proof that all storage has been freed.
5. Observer retirement cancels its outgoing registrations. Entries hold weak observer records, and detached indices contain observation identities rather than strong process owners. A late conversion cannot insert into a closed mailbox. Retained diagnostic handles do not retain application execution.
6. Each cleanup step settles at most eight relationship items and drops at most eight queued mailbox items. The worker services at most eight pending absent-target notifications per turn and continues application work between batches. These are work-item limits, not wall-clock bounds.

Absent-target work uses a removable ordered set: cancellation removes the pending item immediately instead of accumulating stale queue entries. Wake commands are coalesced for pending work; self-monitor and ordinary live registration do not enqueue redundant service commands. These changes address retained metadata and unnecessary queued work by construction, without a speed claim.

## Pinned reference obligations

The clean OTP reference is `ad05823719d77c8faee87348ea39513d4e2f99c5`. This source audit did not execute BEAM or modify its checkout.

| Obligation | Local reference anchor |
|---|---|
| Fresh reference, self-monitor special case, local origin before target send, missing target `noproc` | [Creation](../../upstream/otp/erts/emulator/beam/bif.c:765) |
| An already-exiting target answers a new monitor with `noproc` | [Exit signal dispatcher](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:7347) |
| Cancellation removes the origin before sending target-side cleanup | [Origin removal](../../upstream/otp/erts/emulator/beam/bif.c:488), [demonitor send](../../upstream/otp/erts/emulator/beam/bif.c:522) |
| Down conversion requires an active origin; conversion consumes it | [Permission gate](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6373), [completion](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6490) |
| Info reports origin removal; flush preserves its preselected return result | [Dispatch](../../upstream/otp/erts/emulator/beam/bif.c:639), [single-match helper](../../upstream/otp/erts/preloaded/src/erts_internal.erl:523) |
| Observer death releases outgoing watches; crossing cancellation releases remaining ownership | [Observer exit](../../upstream/otp/erts/emulator/beam/erl_process.c:13891), [cancellation consumer](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6589) |
| Live identity removal precedes monitor traversal; relationship traversal can yield | [Process-table removal](../../upstream/otp/erts/emulator/beam/erl_process.c:14455), [monitor traversal](../../upstream/otp/erts/emulator/beam/erl_process.c:14560) |

Bream's accepted [stop-before-Down contract](../../docs/adr/0009-death-and-reclamation-observation.md) is stronger than BEAM's dirty-native exception. The [shared-envelope decision](../../docs/adr/0007-unified-runtime-envelope.md) already covers the mailbox shape. The representation and batching choices here are reversible implementation choices; no new architecture decision is claimed.

The pinned synthetic-flush behavior is already recorded in [report 29](../../docs/research/29-pinned-reference-observations.md): active ordinary monitor removal can skip the broad tuple-flush helper, whereas an absent-origin flush scans. This milestone qualifies genuine runtime notifications only; it does not resolve that broader typed API mapping.

## Qualification boundaries and Rust opportunities

The retained cohort covers both deterministic creation/death and cancellation/conversion orders, exact-incarnation behavior, independent repeated monitors, self-monitor, observer cleanup, mixed-kind selection and flush, stop-before-Down, and separate reclamation. Six monitor invocations include 96 creation/kill and 96 cancellation/kill races. A test of the actual worker observes a peer poll after the first cleanup batch while final cleanup is still incomplete. These observations do not exhaust every interleaving. The repeated-monitor order assertion uses this prototype’s creation-reference ordering; no universal BEAM order between independent monitors is claimed.

Rust-specific opportunities are explicit ownership of notification variants, opaque checked references, weak observer indices, eager removal of cancelled pending work, and coalesced service notification. Ordered trees still allocate; lock contention, locality and alternative layouts remain unqualified. No historical speed evidence is restored and no performance improvement is inferred.

Links, trapped exits, arbitrary exit reasons, atomic spawn-monitor, aliases, custom monitor tags, priority notifications, registered names, distribution, synthetic Down construction, arbitrary protocols, multiple workers and a real-time timer driver remain outside this profile. G1/G2/G3 and the broader lifecycle gate remain partial; this experiment is not complete BEAM monitor conformance.
