# Receive timeouts with injected time

**Passed for the declared single-worker injected-clock profile, 2026-10-08.** The [final cohort](../../evidence/receive-timeout/runs/20261008-timeout-04/results.json) and [evidence audit](../../evidence/receive-timeout/validation.json) passed.

The fifth milestone extends the [mailbox profile](../mailbox-process/README.md) with zero and finite selective-receive timeouts. Existing `receive`/`receive_any` remain infinite waits. This is a restricted single-worker correctness experiment, with a host-controlled logical clock; it does not contain a real-time timer driver or establish latency behavior. [Milestone tracker](../../docs/milestones.md), [evidence](../../evidence/receive-timeout/README.md).

**No speed tests or performance comparisons may run on this machine.** Runtime time is an injected integer, never sampled host time. Consumer watchdogs detect hangs only. Historical local timings are unsuitable performance indicators. [Persistent instructions](../../AGENTS.md).

## Checked application boundary

The separate consumer uses [generated code](macros/src/structured.rs) and the [runtime](runtime/src/lib.rs):

| Authored operation | Meaning |
|---|---|
| `receive_for(tag, ticks)` | Select the earliest matching fixed tag, or take timeout after waiting. |
| `receive_any_for(ticks)` | Select the oldest ordinary message, or take timeout. |
| `ReceiveResult::Message(message)` pattern | Move the selected concrete envelope into an owning binding. |
| `ReceiveResult::Timeout` pattern | No message selected; retained messages remain available. |
| `receive(tag)` / `receive_any()` | Existing indefinite waits; no finite timer. |

`ReceiveResult` can be held in a private checked record or transferred through a checked helper. Exhaustiveness and duplicate moves are checked by rustc. Constructors, arbitrary predicates, external futures, clock access, custom destructors and general callbacks remain outside the admitted grammar. Tag and duration operands are evaluated once in authored order before the receive future is constructed. Nested match arms retain automatic checkpoints even when rustfmt removes surrounding braces.

Finite durations use logical ticks in `0..=u32::MAX`, corresponding to the reference's finite numeric range. The prototype accepts a `u64` expression and checks the range only if scanning reaches the wait branch. A queued match can therefore complete without validating an unused invalid duration. Invalid waits fail the process after restoring the scan cursor; application exception catching is not admitted. The tick-to-real-time mapping is deliberately unimplemented.

Host `RuntimeClient::advance_to(u128)` publishes monotonic logical time and wakes timer service. Return acknowledges publication, **not completion of all due callbacks**. Backward movement and advancement after shutdown are rejected. Checked deadline addition can fail a process on logical-clock exhaustion without poisoning the locks. This is a prototype host API, not a finalized clock-provider contract.

## Reference behavior and implementation

The clean OTP reference remains `ad05823719d77c8faee87348ea39513d4e2f99c5`. This milestone reviewed source; it did not execute BEAM or modify the checkout.

| Obligation | Reference anchor |
|---|---|
| Evaluate timeout expression once outside scanning | [Compiler binding](../../upstream/otp/lib/compiler/src/v3_core.erl:4342) |
| Scan before reaching wait/arming | [Empty receive transition](../../upstream/otp/erts/emulator/beam/emu/msg_instrs.tab:104), [timer setup](../../upstream/otp/erts/emulator/beam/emu/msg_instrs.tab:317) |
| Zero scans then times out; infinity installs no finite timer | [Branches](../../upstream/otp/erts/emulator/beam/emu/msg_instrs.tab:336) |
| Unmatched wakeups do not restart the timer | [Existing-timer guard](../../upstream/otp/erts/emulator/beam/emu/msg_instrs.tab:317) |
| Expiry applies only to the specific current timer | [Callback arbitration](../../upstream/otp/erts/emulator/beam/erl_hl_timer.c:692) |
| Committed expiry selects timeout at scheduler reentry | [Continuation selection](../../upstream/otp/erts/emulator/beam/erl_process.c:10451) |
| Match cancels; timeout restores the saved prefix | [Removal](../../upstream/otp/erts/emulator/beam/emu/msg_instrs.tab:227), [timeout](../../upstream/otp/erts/emulator/beam/emu/msg_instrs.tab:264), [cancel](../../upstream/otp/erts/emulator/beam/erl_hl_timer.c:2832) |
| Exit cancels receive timers | [Exit cleanup](../../upstream/otp/erts/emulator/beam/erl_process.c:14207) |

[Mailbox](runtime/src/mailbox.rs) and [timer](runtime/src/timer.rs) rules:

1. Scan and budget-yield normally before arming. If the first scan finds a match, no timer is created. If it exhausts, zero takes timeout immediately, finite time arms once, and infinity parks without a timer.
2. Each registration has a checked receive generation plus the complete process identity. Its deadline is computed from logical time at arming, not time at source entry. Unmatched arrivals retain both registration and saved scan position.
3. A deadline index holds weak process references. Successful selection eagerly removes the indexed entry; it does not accumulate cancelled entries until their old deadlines. An already-detached callback must still match the exact registration.
4. The sole application worker commits expiry between polls. It marks the registration expired, clears waiting under the mailbox lock, and posts a full-ID wake. Expiry is checked before any resumed scan, whether ready or parked. A committed expiry can leave an already-queued match for the next receive.
5. Timer service handles at most eight due callbacks per turn. Remaining due work prevents the worker from blocking for commands. Application work and message cleanup continue between service batches.
6. Timeout restores saved nonmatches ahead of the incoming suffix and clears the registration. Kill is checked before application polling. Stop invalidates registrations before releasing the process slot; frame and queued-message cleanup follow.

Arming/cancellation acquires mailbox then clock. Service releases clock before acquiring mailbox, preserving the registry → lifecycle → mailbox order used by stop. Timer entries never own process records strongly. A retained diagnostic handle or delayed callback does not retain an application frame.

The reference can also run a timer callback concurrently with an already-running match; that match may cancel even a committed timeout sentinel. This prototype commits callbacks only between polls, and does not claim coverage of that concurrent in-poll branch. Internal tests deliberately detach callbacks to challenge stale identity, even though the current single worker services a detached callback immediately.

## Evidence and remaining work

The retained cohort and independent review cover 35 internal tests per compiler profile, inherited mailbox controls, new timeout controls, 75 negative consumers per profile and four positive shadowing routes. Timed controls include zero scanning across turns, unmatched arrivals, cancellation/rearming, owning results across suspension, actual invalid/overflow failure cleanup, kill, shutdown, slot reuse, 19-timer bursts and bounded send/timeout and kill/timeout races. All oracles concern allowed outcomes, ownership or logical scheduling work.

The nested-match regression is also retained: an initial build rejected rustfmt's equivalent form; the first syntax fix omitted its checkpoint; a new work-count assertion exposed that omission before the final fix. See the evidence directory for the exact failed and superseded inputs, rather than treating the earlier passing cohort as qualified.

Structural Rust opportunities are explicit: concrete owning result variants, checked identity construction, weak timer-index records, and immediate removal of cancelled entries. No speedup is inferred. The deadline tree still allocates nodes; locality and timer-backend tradeoffs remain unqualified.

G1/G2/G3 remain partial. A real monotonic-clock driver, worker-clock integration, broader resource/failure recovery, richer protocols, timer-only receive syntax and complete runtime qualification remain open. Next is a bounded monitor/Down milestone: monitor creation versus target death, immediate absent-target notification, cancellation and owner cleanup, with Down ordered after irreversible execution stop and separate final reclamation.
