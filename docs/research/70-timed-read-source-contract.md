# Timed persistent reads: source contract

**2026-10-09. Source audit and proposed milestone contract. No experiments, benchmarks or performance comparisons ran for this report.** OTP baseline: **29.1.1**, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`, inspected from the pinned checkout. Links below identify that exact source. The [preceding implementation](69-persistent-read-milestone.md) supplies persistent local endpoints and distinct request identities; this report adds the obligations of a deadline.

Subsequent implementation and execution status is recorded separately in [report 71](71-timed-read-milestone.md).

The central finding is that the pinned Unix socket implementation does **not** asynchronously interrupt an in-progress `recv` when its timeout expires. The receive NIF and cancellation both acquire the descriptor's `readMtx`; the nonblocking syscall and result processing occur before receive releases that lock. Bream's driver-owned `Running` phase must provide the corresponding exclusion while leaving the application worker free to run. [Receive serialization](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/nifs/common/prim_socket_nif.c#L7096-L7120), [cancel serialization](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/nifs/common/prim_socket_nif.c#L13969-L13977), [actual syscall](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/nifs/unix/unix_socket_syncio.c#L3817-L3864).

## What the pinned wrapper establishes

| Branch | Reference fact | Consequence for the bounded Rust profile |
|---|---|---|
| Finite timeout | Setup derives one monotonic deadline; retries retain it and derive remaining waiting time. | Capture the injected-clock deadline once, without restarting it on readiness, unrelated wakeup or `WouldBlock`. |
| Zero timeout | A separate `recv_zero` path polls the OS without asynchronous registration. | Permit one nonblocking driver attempt and no timer or readiness wait. |
| Infinity | No finite deadline; receive may wait for data. | Keep the existing `read_next()` indefinite wait. |
| Positive stream length | The wrapper accumulates input until the requested length or timeout/error; timeout can carry partial data. | Bream still returns the first positive short read. It does not implement read-exactly or a partial-data timeout result. |
| Successful syscall | An immediate successful result returns without a subsequent deadline check. | A universal rule that expiration defeats every result completed after the timestamp would be an invented contract. |

These branches are explicit in the [receive contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/kernel/src/socket.erl#L5910-L5956), [zero implementation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/kernel/src/socket.erl#L6088-L6106), [deadline construction](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/kernel/src/socket.erl#L8101-L8136), [direct success](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/kernel/src/socket.erl#L6154-L6199), and [same-deadline retries](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/kernel/src/socket.erl#L6270-L6313).

There is a further deadline subtlety: the select branch computes `Timeout` before entering its receive and uses that captured value when deciding whether a received readiness notification permits retry. The retry performs another primitive receive before deriving another remaining wait. This source shape supports neither a strict completion timestamp cutoff nor a wall-clock return bound. Readiness, syscall completion, timer service and application dispatch are distinct events. Bream should make its own permitted ordering explicit, rather than claim that the OTP wrapper prescribes one universal winner.

## Cancellation is qualified by operation and resource lifetime

Unix cancellation checks both calling PID and current operation reference. An obsolete reference cannot cancel whichever read happens to be current. Cancelling the matching current reader cancels its selection and advances the reader queue. Retry can replace the current reference while retaining the same reader process. [Identity check and advancement](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/nifs/unix/unix_socket_syncio.c#L5255-L5354), [retry reference replacement](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/nifs/unix/unix_socket_syncio.c#L10419-L10449).

The readiness-cancel result distinguishes actual cancellation from a notification already sent. The Erlang wrapper separately flushes available select/completion/abort messages for the particular socket and handle. This is scoped cleanup, not authority for an old notification to act on a new request. It is also not a general proof that every concurrent native callback has finished. [Native cancel result](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/nifs/common/prim_socket_nif.c#L14135-L14164), [operation-scoped flushing](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/kernel/src/socket.erl#L8040-L8082).

Descriptor destruction has a stronger, separate boundary: `ERL_NIF_SELECT_STOP` invokes or schedules the resource stop callback when the event can safely close. The resource association survives until that callback returns, even after notifications are received or cancelled. Bream retains its different mechanism: only the driver may close endpoint descriptors, and captured poll snapshots remain valid until the driver has finished using them. Expiring a read does not close its persistent endpoint. [Pinned NIF lifetime contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/references/erl_nif.md#L3566-L3594), [existing Bream ownership contract](68-persistent-read-source-contract.md#descriptor-lifetime-admission-and-the-rust-opportunity).

## Proposed Bream arbitration

The following is a **narrower Bream policy informed by the source**, not exact `socket:recv` equivalence. Use one endpoint-state lock to arbitrate completion and expiry for the exact request identity. Keep the existing one-owner, one-application-worker, one-driver and nonblocking local-stream restrictions.

- **Pending expiry:** commit `Timeout`, retire that request's buffer and preserve the endpoint. No later driver claim may consume bytes for this request.
- **Running expiry:** record timeout intent and request that the driver skip the syscall if it has not passed its cancellation check. Preserve the claimed endpoint and buffer, and keep the future pending until this attempt settles. Do not expose an apparently reusable endpoint before settlement.
- **Claimed result after expiry:** a positive `Data`, EOF or non-retryable error wins. A `WouldBlock`, `Interrupted` or skipped attempt yields `Timeout` once timeout intent exists. A successful read is therefore never silently discarded merely because its timer was serviced while the syscall was in progress.
- **Ready before expiry:** preserve the already committed result. Timer service must not replace it.
- **Stop or future destruction:** these remain cancellation of application delivery. They may discard a claimed result after the external read has consumed bytes. Timeout's claimed-result preference must not override process death.

After the actor receives `Timeout`, the old request must already have settled; a subsequent read can immediately obtain a fresh request identity on the preserved endpoint. This improves the usability of the sequential API at the cost of allowing a claimed read to delay the timeout result until syscall settlement. It supplies no hard response-time guarantee. A forced scheduler pause, a blocking host alias or driver failure cannot be dismissed by calling the operation nonblocking.

Zero should mean one attempt when the driver processes the request, even if that requires suspending the actor. It does **not** promise a syscall at the instant the actor invoked the checked operation. Unlike OTP's zero branch, Bream proposes treating `Interrupted` as `Timeout`; the pinned Unix branch special-cases only `EAGAIN`/`EWOULDBLOCK` and handles other errors generically. This is an explicit simplification. Finite deadlines retain the existing checked duration range and overflow rejection; indefinite reads arm no timer. [OTP error distinction](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/nifs/unix/unix_socket_syncio.c#L10145-L10208).

Reuse the [existing injected clock](../../experiments/persistent-read/runtime/src/timer.rs) while distinguishing timer destinations. A detached callback must validate its exact generation, owner identity and active read identity; process identity alone does not suffice. Timer removal and completion must not introduce opposite lock ordering between clock, mailbox and endpoint state. Completed or stopped reads should eagerly remove registrations rather than accumulate cancelled timers until their original deadlines. These are implementation obligations, not evidence already collected by this source pass.

## Required correctness controls and reference cases

Bream controls should force pending expiry, ready-before-expiry, expiry before the syscall's cancellation check, positive completion after that check, and `WouldBlock` settlement after expiry. Check that timeout remains pending while the driver owns a claimed request, then reuse the endpoint and retain earlier delivered bytes. Deliver a detached old timer, old readiness ticket and old completion after a new request exists. Preserve one deadline through retries. Cover zero data/empty/error paths, infinity without timer allocation, peer progress, stop, same-slot replacement, shutdown and timer/endpoint/buffer balances. Use barriers, logical clock advancement and counters; watchdogs only detect hangs.

A small **proposed, not executed here** BEAM fixture can create local stream pairs and assert these results without measuring duration:

1. Empty zero receive returns timeout; buffered exact-length zero receive returns the bytes.
2. Empty finite receive times out; a subsequent send and receive succeed on the same endpoint.
3. Infinity returns buffered data.
4. Cancel asynchronous handle A, start B, reject stale cancellation A, and receive B's data.
5. A deliberately larger positive requested length with zero timeout returns partial bytes alongside timeout. Record this as a semantic difference from Bream's short-read success.

Linux abstract addresses avoid pathname cleanup; the pinned API documents zero-prefixed local addresses. A finite reference case necessarily uses OTP's real timer, but asserts only timeout and reuse outcomes—no clock reads, elapsed values or latency bounds. Such a fixture would not reproduce Bream's injected-clock interleavings, prove all cancellation races, or constitute a performance comparison. [Local address contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/lib/kernel/src/socket.erl#L927-L947).

The Rust opportunity is an explicit typed timer destination and move-only request storage, with one completed-buffer transfer and no extra timeout payload copy. Any allocation or reference-count reduction remains a candidate for separately authorized performance qualification. This report establishes source and design boundaries only.
