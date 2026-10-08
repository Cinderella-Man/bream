# Local links and trapped Exit notifications

**Passed for the restricted local single-worker profile, 2026-10-08.** This seventh bounded milestone extends [local monitors](../monitor-process/README.md) with bidirectional links, receiver exit trapping and genuine `Exit` events in the shared mailbox. It remains a native Rust experiment with an injected logical clock. [Final correctness cohort](../../evidence/link-process/runs/20261008-link-02/results.json), [read-only audit](../../evidence/link-process/validation.json), [milestone tracker](../../docs/milestones.md).

**No speed tests or performance comparisons may run on this machine.** Logical events and injected ticks establish semantic order only. Watchdogs detect hangs; their elapsed times are not performance evidence. Performance qualification requires a separately authorized suitable environment. [Persistent instructions](../../AGENTS.md).

## Checked application boundary

The separate consumer enters through the [checked grammar](macros/src/structured.rs) and [runtime context](runtime/src/lib.rs). These operations are experimental authored syntax, not a finalized library interface.

| Authored operation or pattern | Meaning |
|---|---|
| `link(pid)` | Establish one bidirectional link. Self-link and repeated active link are no-ops. An already absent target with trapping disabled raises the restricted uncaught `noproc` failure synchronously; application code after the call does not execute. |
| `try_link(pid)` | Perform the same operation with explicit Rust recovery: `LinkResult::Linked` or `LinkResult::NoProcess`. The latter reports the synchronous absent-target case rather than terminating the caller. |
| `unlink(pid)` | Remove the current relationship and suppress its future signals unless it is re-established. Absent links are no-ops. Already converted mailbox events remain. |
| `trap_exit(enabled)` | Change the receiver's trap flag and return its previous value. New processes start with trapping disabled. |
| `receive_exit(pid)` / `receive_exit_for(pid, ticks)` | Select the earliest genuine Exit from that exact process identity, retaining other events; the finite form can time out. |
| `exit_from(exit)` / `exit_reason(exit)` | Read the sender identity and restricted reason code. |
| `Event::Exit(exit)` | Inspect a linked-exit event alongside `Event::Message` and `Event::Down`. |
| `ExitResult::Exit(exit)` / `ExitResult::Timeout` | Inspect the finite Exit-selective receive result. |
| `receive_event()` / `receive_event_for(ticks)` | Select the oldest event across application messages, Down and Exit notifications. |

Existing monitor, application-message and injected-timeout operations remain available. `receive_any()` selects application messages only; it skips both Down and Exit events. `Exit` fields are private, and the checked grammar does not admit construction or reinjection of lifecycle notifications. Explicit recovery through `try_link` does not add general exception catching.

Reason codes are deliberately restricted: **1** normal completion, **2** killed, **3** process failure, **4** `noproc`. They are not arbitrary Erlang terms. The diagnostic snapshot separates execution classification (`terminal`) from the committed lifecycle reason (`exit_reason`). For example, an untrapped linked reason `killed` can fail the receiver's execution while its propagated reason remains `killed`.

Trusted host controls expose `RuntimeClient::link`, `unlink`, `set_trap_exit` and an active-link count. The origin of link creation must be live. An absent or stale target with trapping disabled returns `LinkError::NoProcess` synchronously. A trapping origin instead installs pending linked `noproc` work; unlink can remove that work before conversion. A foreign-runtime target is absent in this local profile, not distributed routing.

There is no authored direct-exit operation in this milestone. Existing host kill remains untrappable, stops the addressed process and propagates the resulting `killed` reason through links. A trapping peer can receive that propagated reason as an Exit event. Legacy `exit/2`, modern `exit_signal/2`, catchable `exit/1`, arbitrary exit reasons and a literal linked reason `kill` remain outside the qualified interface.

## Relationship and signal transitions

[Link storage](runtime/src/links.rs), [mailbox selection](runtime/src/mailbox.rs) and [retirement](runtime/src/lifecycle.rs) implement distinct transitions:

1. Link publication and either endpoint's retirement serialize through the registry. One canonical pair of complete process identities represents the relationship; endpoint records are weak references. Full runtime, slot and generation identity prevents retired work from affecting a replacement process in a reused slot.
2. Normal untrapped linked exit removes the relationship and otherwise has no effect. A trapped eligible signal becomes an Exit event. An abnormal untrapped signal commits pending termination, wakes a parked receiver and prevents another application poll; application receive is not needed for termination.
3. The receiver's trap flag is read at conversion, not captured at link creation or sender death. Flag changes, unlink and conversion serialize through the link table. Changing the flag after conversion does not reinterpret an existing mailbox event.
4. Unlink suppresses an unconverted linked signal by removing its current relationship. It does not flush an existing Exit. Re-establishment can make an older linked signal applicable again, matching the reference's qualification of its unlink guarantee. Retired work uses the pair of full process identities rather than a separate link incarnation token.
5. Retirement closes the mailbox, cancels its timer, removes live identity and commits the reason before notification conversion. Detaching a per-process relationship index does not walk every relationship. Later cleanup removes reciprocal indices as it settles each item. This is an ordered-tree implementation, not a constant-time lookup claim.
6. Link notifications convert synchronously within a cleanup step before that process's monitor cleanup starts. For an ordinary observer with both relationships to the same target, the trapped Exit precedes its Down. Application messages already sent by that target precede the Exit in the shared queue. These ordering statements do not promise consumption before fatal termination.
7. Each retirement step settles at most **eight combined link and monitor items**, with links first, and releases at most eight queued mailbox items. Separate pending absent-target service handles at most eight link items and eight monitor items per worker turn. Application work can run between cleanup batches. These are work-item limits, not wall-clock bounds.

The [stop-before-Down decision](../../docs/adr/0009-death-and-reclamation-observation.md) also governs linked notifications: admitted application execution has stopped before its death notification becomes visible. Receiving Exit or Down is not proof of final reclamation. Dead endpoints, crossing retirement and cancelled absent-target work release relationship metadata without retaining application execution.

Finite Exit receives inherit the existing logical-clock rules: `0..=u32::MAX` ticks, scanning before waiting, zero-timeout scanning, eager timer cancellation on selection and exact receive-generation checks. There is no real-time clock driver.

## Pinned reference obligations

The clean OTP reference is `ad05823719d77c8faee87348ea39513d4e2f99c5`. The source audit inspected these branches without executing BEAM:

| Obligation | Local reference anchor |
|---|---|
| Self and repeated active links are no-ops; absent local untrapped link takes the synchronous error branch | [Link setup](../../upstream/otp/erts/emulator/beam/bif.c:155), [error construction](../../upstream/otp/erts/emulator/beam/bif.c:410) |
| Unlink deactivates the local relationship; old acknowledgments cannot remove a newer active relationship | [Unlink publication](../../upstream/otp/erts/emulator/beam/bif.c:1223), [acknowledgment check](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6758) |
| Unlink leaves converted Exit messages and excludes relink from its future-effect guarantee | [Documented guarantee](../../upstream/otp/erts/preloaded/src/erlang.erl:6852) |
| Linked exit checks current relationship state and removes it before trap/ignore/fatal classification | [Conversion gate](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:4674) |
| Trap state at handling determines conversion; direct kill alone becomes untrappable killed | [Classification](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:4738), [trap flag setter](../../upstream/otp/erts/emulator/beam/bif.c:2058) |
| Link cleanup sends the committed reason and can yield before monitor traversal | [Link dispatch](../../upstream/otp/erts/emulator/beam/erl_process.c:14068), [cleanup phases](../../upstream/otp/erts/emulator/beam/erl_process.c:14545) |
| Signals sent by one entity to one receiver retain their order | [Signal ordering](../../upstream/otp/system/doc/reference_manual/ref_man_processes.md:507) |

These obligations continue the accepted [exit/trapping semantics](../../docs/adr/0006-beam-exits-and-trapping.md) and [BEAM-default rule](../../docs/adr/0010-beam-observable-semantics-by-default.md). [Report 16](../../docs/research/16-lifecycle-edge-cases.md) records the missing-target, relink and self-exit distinctions; [report 20](../../docs/research/20-process-lifecycle-deep-dive.md) records the ordinary Exit-before-Down source prediction. A shared registry-serialized relationship table replaces OTP's asynchronous endpoint protocol within this profile; the implementation does not claim exhaustive crossing-signal conformance.

## Qualification boundaries and Rust opportunities

The final [correctness cohort](../../evidence/link-process/README.md) passed all **263 expected command outcomes**: **68 unit tests per debug/release profile**, six public link configurations, inherited mailbox/timeout/monitor controls, **210 expected compilation rejections** and **twelve positive shadowing executions**, plus builds. Link configurations include **96 link/kill and 96 unlink/kill races** with allowed-outcome assertions; no particular race winner is required.

Deterministic tests force trap changes, cancellation before/after conversion, both same-identity relink orders, reused-slot isolation, crossing retirement, shared queue ordering and finite selection. An actual-worker test commits fatal exit before dispatch and observes zero subsequent application polls. Another observes peer execution after the first cleanup batch. The combined-budget test crosses from two remaining links to six monitors in its second batch, leaving four monitors for a third batch. The first passing cohort is retained; final review strengthened this cross-phase assertion in the second. These bounded observations are not an exhaustive interleaving or weak-memory proof.

Rust-specific opportunities are explicit notification variants, weak endpoint ownership, one shared edge per pair, eager removal of cancelled absent-target work and coalesced pending-work wake commands. These choices constrain ownership and queued metadata by construction. Ordered trees still allocate; contention, locality, alternative representations and speed remain unqualified. No historical performance evidence is restored.

Arbitrary exit terms, authored direct exits, atomic spawn-link/monitor, registered names, distribution, ports, aliases, priority messages, synthetic notification look-alikes, multiple workers, general exception handling and a real-time timer driver remain outside this profile. Broader lifecycle gates remain partial; this milestone is not complete BEAM conformance.
