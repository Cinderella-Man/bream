# Owned mailboxes, selective receive and parking

**Passed for the declared single-worker profile, 2026-10-08.** Generated native processes now move owned messages, receive the earliest matching tag, retain nonmatches and park without busy polling. Send or kill can wake a parked process. Stop closes the mailbox and cleans queued ownership in bounded batches, separately from the stopped observation.

**Correctness evidence only.** The user concurrently uses this machine; speed tests and performance comparisons are prohibited. This runtime contains no clocks or latency instrumentation. Consumer watchdogs only detect hangs. No throughput, latency, speedup or production allowance is inferred. [Persistent machine instructions](../../AGENTS.md), [milestone tracker](../../docs/milestones.md), [retained evidence](../../evidence/mailbox-process/README.md).

## Runnable boundary

The separate [consumer](consumer/src/main.rs) exercises the [checking compiler](macros/src/structured.rs) and [runtime](runtime/src/lib.rs). The supported application envelope is one concrete schema: a scalar tag and moved `Words` buffer. Mailbox length is not capped. Individual buffer capacity remains limited to 8,192 words by the admitted experimental resource profile; allocation failure is not recoverable here.

The generated module supports:

| Authored operation/type | Meaning |
|---|---|
| `Pid`, `self_id()`, `input_target()` | Concrete namespace/slot/generation identity. Target defaults to self or is supplied by host `spawn_to`. |
| `send(pid, tag, words)` | Move the buffer into a runtime envelope. Ordinary dead/stale IDs silently discard it; return is not application-receipt acknowledgment. |
| `receive(tag)` | Suspend until the earliest message with that fixed scalar tag is selected. Evaluate the selector once before suspension. |
| `receive_any()` | Select the oldest ordinary message. |
| `Message`, `message_tag(message)` | Concrete owning envelope; observe its tag without moving its payload. |
| `message_words(message)` | Consume the envelope and move out its buffer. Rust rejects subsequent reuse. |

These operations compose with the previously checked records, helpers and shared process allowance. The compiler reconstructs paths and temporaries, preserves argument order and introduces automatic suspension. No arbitrary callback predicate, custom destructor, external future, explicit application await or general type erasure is admitted. Host `Envelope::new` accepts only concrete buffers and returns an independent allocation receipt for ownership assertions.

This is one typed application-envelope profile, not arbitrary user protocols, full pattern guards or the accepted future envelope containing runtime notifications. Foreign-runtime IDs are discarded by this local-only send API; cross-runtime routing is outside the profile.

Run the retained controls, with a declared logical scheduling configuration:

```sh
.tools/mailbox-process/20261008-mailbox-01/debug/bream-mailbox-consumer controls 4
```

Only allowances 1, 4 and 32 belong to this control fixture. The fixed long-prefix case must force a scheduling yield. This is a logical work oracle, not a timing comparison.

## Ownership and scheduling protocol

[Mailbox implementation](runtime/src/mailbox.rs), [lifecycle implementation](runtime/src/lifecycle.rs):

1. Resolve the complete target ID under registry protection and retain the exact record. Under its mailbox lock, append the moved envelope or observe closed ingress and discard it. A lookup that preceded stop cannot publish into the closed old record.
2. Receive checks at most its remaining allowance of candidates. Each rejected candidate moves to a saved prefix; this prefix survives budget yields and unmatched wakeups. Selection restores the saved prefix before the unexamined suffix for the next receive.
3. With unexamined input and exhausted allowance, yield as runnable. With no unexamined input, mark waiting under the same lock used by publication. A mailbox containing old nonmatches may correctly park.
4. Send clears waiting under that lock and leaves a full-ID wake command when necessary. The sole worker owns ready and parked task containers; an early wake leaves the task runnable, a late wake removes the parked task, and duplicate/stale wakes cannot create another execution owner or target a replacement.
5. Kill retains the previous terminal arbitration and also wakes waiting work. The worker tests kill before application polling. Stop follows registry → lifecycle → mailbox lock order, closes ingress, detaches both lists, removes the live identity and publishes a logical stopped event.
6. Frame destruction outside those locks reclaims already-selected messages and local state. Detached queued envelopes are destroyed in batches of at most eight per worker turn, interleaved with application turns. Cleanup completion follows the final batch. The worker does not block for commands while cleanup remains, and shutdown includes parked tasks.

Counters follow allocation origin independently of current process ownership. A sent buffer may outlive its sender's stopped frame: the receiver now owns it. Its allocation receipt contains no back-reference to a process or mailbox, so self-send cannot create that retention cycle. A stopped sender's allocation counters may change when the receiver later releases data; application execution counters remain stopped. Cleanup completion does not revoke ownership already transferred to another live process.

The mailbox keeps saved and incoming linked lists so restoring a long prefix does not shift every retained envelope. Rejected candidates transfer existing nodes with `split_off(1)` and `append`; a correctness test checks node addresses across scan and restoration. Rust's [LinkedList documentation](https://doc.rust-lang.org/std/collections/struct.LinkedList.html#method.append) describes node reuse for append. This is a structural choice, not a speed claim: ingress still allocates a list node and locality/allocation tradeoffs remain unqualified.

## BEAM obligations revisited

The clean reference remains OTP `ad05823719d77c8faee87348ea39513d4e2f99c5`. Source was inspected; no new BEAM executable was run.

| Obligation | Pinned source |
|---|---|
| Earliest matching ordinary message, preserving nonmatches | [Language contract](../../upstream/otp/system/doc/reference_manual/expressions.md:680) |
| Charge failed matching and retain the scan position across yield | [Receive instructions](../../upstream/otp/erts/emulator/beam/emu/msg_instrs.tab:249) |
| Carry final empty check into the parking decision | [Receive-helper contract](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:7739), [wait](../../upstream/otp/erts/emulator/beam/emu/msg_instrs.tab:371) |
| Publish or discard under queue ownership, then notify | [Direct message ingress](../../upstream/otp/erts/emulator/beam/erl_message.c:405) |
| Keep wake activity across schedule-out | [Activation](../../upstream/otp/erts/emulator/beam/erl_process.c:6899), [schedule-out](../../upstream/otp/erts/emulator/beam/erl_process.c:6741) |
| Kill is runtime control independent of receive matching | [Signal handling](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:4738) |
| Collect incoming ownership for exit cleanup | [Exit collection](../../upstream/otp/erts/emulator/beam/erl_process.c:14534), [queue cleanup](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:9588) |

The Rust single-worker implementation uses a locked publication/parking decision and full-ID wake commands; it does not copy ERTS's buffered ingress, multiworker state bits or receive markers. [Existing parking analysis](../../docs/research/31-receive-parking-wakeup-protocol.md) retains those wider branches.

## Evidence and limits

All **155 expected command outcomes** passed in the [cohort](../../evidence/mailbox-process/runs/20261008-mailbox-01/results.json): 21 internal tests per compiler profile, six public controls, 132 expected compilation rejections and six positive shadowing executions, plus builds/compiler identity. The separate [validator](../../evidence/mailbox-process/validation.json) checks copied inputs, raw outcomes and unchanged prior evidence.

The [internal tests](runtime/src/lifecycle/tests.rs) retain the identity/publication cases and add explicit send-before/after-park orders, actual wake publication, duplicate/stale wakes, kill of parked work, retained lookup after close, scan cursor/order/budget, allocation-record separation, batch destruction and node preservation.

Each public invocation checks four-message selective order, idle parked behavior, unmatched arrivals without rescanning, held-plus-queued cancellation, self-send, data outliving its sender, generated forwarding, two concurrent senders with per-sender order, stale/foreign discard and 16 bounded send/kill races. The long scan examines exactly 129 candidates while a peer completes first. A 65-message stopped mailbox takes nine cleanup batches, with a peer's completion logically between stop and cleanup. A 33-message mailbox drains after kill even with no runnable application peer. These assert logical ordering, not elapsed time.

The 66 compile-failing fixtures per profile include the 57 inherited cases and nine new message cases: duplicate moves, unsupported predicate, forged/numeric destination, PID arithmetic, envelope construction/cloning, built-in shadowing and custom payload. Three lexical-shadowing programs execute in both profiles. Negative fixture counts group some message concerns in the same source case; they are not exhaustive language qualification.

Remaining work includes receive timeouts, richer checked message protocols/patterns, runtime notifications/relationships, broader resource limits, allocation failure and unexpected worker-failure recovery. Snapshot fields are diagnostic observations, not an atomic transaction over every counter. Default allocator behavior and admitted frame destruction still prevent a hard wall-clock bound. G1/G2/G3 remain partial. The next finite milestone is receive timeout correctness with an injected clock, so no speed or real-time latency experiment is needed.
