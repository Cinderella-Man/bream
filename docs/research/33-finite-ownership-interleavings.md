# Finite interleavings: retained records and receive parking

Research executed 2026-10-07. Source reference: OTP 29.1.1, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. This pass explores **two small, sequentially consistent protocol abstractions**, with five deliberately broken variants. It does not execute BEAM, translate all its code, establish Rust soundness or qualify Bream. The earlier [reference VM observations](29-pinned-reference-observations.md) remain separate evidence.

The question is narrower and useful: which ordering constraints must a future implementation preserve, and can removing one produce a concrete failure even in a very small model? The [explorer](../../evidence/process-interleaving-review/explore.py) and [raw results with shortest counterexamples](../../evidence/process-interleaving-review/model-results.json) are retained. No external model-checking package is required.

## Method and evidence boundary

The explorer performs breadth-first search over immutable states, retaining every distinct reachable state of a baseline model. Each transition is one explicitly assumed atomic event. No random scheduler, elapsed-time assumption or timeout controls the result. A broken variant stops at its first counterexample, so its state count is not a complete graph count. Baseline terminal states are also checked for completion of their stated protocol obligations.

| Model | Actors and bounds | Baseline exploration | Broken variants detected |
|---|---|---|---|
| Record retention | One lookup reader, one exiting process, one record and one optional retained reference | 41 states, 60 transitions, two terminal states | Guard released before pin; grace period skipped; record reference used as heap permission |
| Direct receive parking | One receiver currently running, one external sender, one fresh message, one normal-priority queue reservation | 44 states, 64 transitions, two terminal states | ACTIVE cleared after unlocking; schedule-out uses stale store instead of retrying CAS |

These counts describe these models, not BEAM paths, real machine schedules, or percentages of feature coverage. Both baseline graphs exhaust without their modeled violations; all five negative controls produce counterexamples. The script records its own hash and Python/platform identity.

## 1. Borrow-to-reference handoff

The modeled reader enters temporary lifetime protection, looks up the published record, acquires its retained reference while protected, leaves temporary protection, inspects permitted metadata, then releases its reference. The exiting side unpublishes the record, reclaims its application body, and eventually releases the baseline reference after the modeled grace condition. Physical record release occurs when neither baseline nor reader reference remains.

This projection follows the following actual source constraints:

| Model event | Pinned source correspondence | Deliberate abstraction |
|---|---|---|
| Enter → lookup → increment → leave | [Unmanaged protected handoff](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.c#L1000-L1028) | A boolean guard replaces real thread-progress cohorts and barriers. |
| Lookup only a published incarnation | [Raw lookup and full PID comparison](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.h#L1135-L1156) | One incarnation; slot reuse, wrap and allocator-address reuse are excluded. |
| Unpublish before retirement | [Removal under process locks](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14429-L14479) | Application execution has already stopped; the kill transition itself is not modeled. |
| Body lifetime differs from record lifetime | [Ordinary versus delayed body deletion and baseline retirement](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14755-L14791), [final record release](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12139-L12149) | Only ordinary body cleanup is represented. The later callback's execution is abstracted after body reclamation; registering that callback earlier in C is not equated with running it. |
| Increment presupposes a valid record | [Reference operations](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.h#L368-L384) | Reference arithmetic is represented by ownership booleans, with no weak-memory execution. |

The first broken handoff gives this minimal failure:

```text
reader enters protection and finds record
reader releases protection too early
exit unpublishes, frees body, and releases baseline after grace
reader increments a reference on the freed record
```

The grace-period control fails even earlier: a reader still borrowing the record loses it when retirement ignores its protection. Conversely, the correct model deliberately permits the state **record retained, body already reclaimed**. The third negative control accesses the heap in that state and fails. This is a distinction between permissions, not evidence that correctly retained metadata itself is unsafe. [Report 27](27-process-lookup-reclamation-protocol.md) supplies the broader lookup/lock/lifetime trace.

**Bream implication:** an identity/metadata handle, permission to access live process state, and ownership of a separately retained resource must be distinct contracts. A reference-counted control block alone cannot establish all three. This informs LIFE-05.C14/C16/C19; it does not select a reference-counting or epoch implementation.

## 2. Final receive check, inactivity and schedule-out

The receiver's final empty check holds MSGQ. It clears ACTIVE before releasing that lock; RUNNING remains set until schedule-out. The sender appends while holding MSGQ, releases the lock, then notifies. Notification and schedule-out must agree on who reserves and later physically publishes a queue entry. [Final fetch retains the lock](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7738-L7798), [interpreter wait](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/emu/msg_instrs.tab#L371-L385), [JIT wait](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/beam_jit_common.cpp#L1190-L1206).

Three model details are essential:

1. **Append and notification are separate events.** The receiver can observe input before its sender finishes notifying. Such a later notification may cause redundant activation, which is not a lost wakeup. [Direct publication order](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_message.c#L406-L459).
2. **Reservation and physical queue insertion are separate events.** A transient reserved-but-not-yet-inserted entry has an explicit publishing owner; it is not automatically lost work. [State arbitration](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L6935-L6975), [queue insertion](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L6830-L6855).
3. **Schedule-out retries a changed state.** The model separates reading the state from the compare-and-swap that clears RUNNING and reserves any necessary queue entry. [Schedule-out loop](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L6766-L6803).

The modeled failure condition is **fresh, unexamined input after both actors finish, with no running owner, queue entry or outstanding notification**. It is not merely “mailbox nonempty”: selective receive may legitimately sleep with already-examined unmatched messages. The successful observation branch ends this model when the current receiver sees the fresh input; it does not simulate the entire subsequent receive or application computation.

Moving the inactive transition outside its lock produces:

```text
receiver checks empty, then unlocks prematurely
sender appends and notifies while ACTIVE is still true
receiver clears ACTIVE and schedules out
fresh input remains, but nobody has queued the process
```

Replacing schedule-out CAS/retry with a blind stale store produces a different failure:

```text
receiver clears ACTIVE under lock, unlocks, then snapshots inactive state
sender appends and activates the still-RUNNING receiver
receiver overwrites that activation using its stale snapshot
no queue entry is published
```

**Bream implication:** “put a message in a concurrent queue, then wake the task” is incomplete as a design. Parking, activation, execution ownership and queue-publication ownership form one protocol. The model supplies small counterexamples for EXEC-01.C05 and MSG-03.C05; [report 31](31-receive-parking-wakeup-protocol.md) traces the actual wider path, including timer and buffered-input handshakes.

## 3. What has not been proved

The models assume sequential consistency, faithful actor steps, correct mutex behavior and successful allocation. They do not explore C/Rust memory models, refcount overflow, full-ID reuse, multiple readers/senders, proxy entries, worker migration, worker failure, teardown of the host runtime, allocator behavior, dirty native execution, or user destructors. The parking model additionally excludes timers, buffered ingress, exit/control signals and arbitrary selective-receive scans. It ends before scheduler dequeue/claim by another worker, so its queue-ownership checks do not independently establish execution exclusivity. Its successful notification update is one atomic abstract event; its internal CAS retries are not modeled separately. No fairness or real-time bound follows from a finite safety graph.

The source traces themselves reveal why the missing products matter: buffered senders use a different MSGQ synchronization rule, and receiver death does not itself revoke every previously acquired publication path. [Report 30](30-buffered-signal-publication-protocol.md) records those obligations, including an unclosed reinstallation concern. That concern is not a reproduced defect and is outside these two models.

Next experiments should add one missing interaction at a time with an explicit source-to-model mapping: buffered enqueue/deinstallation and wakeup; receive timer identity versus cancellation; then relationship cancellation against exit cleanup. A future implementation also needs controlled pause points at the actual operations, checked resource accounting and a Rust ownership argument. Neither these models nor more reference stress runs solve admission of arbitrary native dependency calls.

## Reproduction

From the Bream repository root:

```bash
python3 evidence/process-interleaving-review/explore.py
```

This overwrites only this model's result JSON. The [evidence index](../../evidence/process-interleaving-review/README.md) records scope and validation. All relevant Bream features remain planned.
