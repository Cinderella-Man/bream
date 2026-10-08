# Relationship cancellation: local suppression, asynchronous ownership

Source review: 2026-10-07; OTP 29.1.1, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. **No new VM runs, tests, source changes or Bream implementation were performed for this report.** Existing executable observations are identified separately in [report 29](29-pinned-reference-observations.md). This extends [the lifecycle audit](16-lifecycle-edge-cases.md) and [endpoint/alias review](20-process-lifecycle-deep-dive.md).

Both cancellation calls establish a caller-side barrier without waiting for the peer to finish cleanup. Their protocols differ: unlink keeps an inactive endpoint until correlated completion or another terminal event; demonitor removes permission to create a notification immediately and transfers endpoint ownership asynchronously. Neither cancels the other process or retracts already accepted application messages.

## 1. What return establishes

| Operation | Established when it returns | Not established |
|---|---|---|
| `unlink(P)` | The removed link cannot subsequently affect this caller, unless established again. | Peer already handled removal; existing trapped EXIT was removed; direct exit signals from P are suppressed. |
| `demonitor(R)` | No later mailbox insertion caused by this monitor. | Existing DOWN was removed; target stopped; both endpoint allocations have been reclaimed. |
| `demonitor(R,[info]) = true` | Ordinary origin was found and removed; its genuine DOWN was not previously delivered and cannot be delivered later. | Target is currently alive. |
| Ordinary `[flush,info]` with absent origin | Run the matching receive helper, return false. | A tuple actually existed or was removed. |

These distinctions come from the [unlink contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L6852-L6877), [monitor contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L2024-L2055) and [actual info/flush dispatch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L639-L662). The documentation's informal “false if a flush was needed” must not become a message-removal count: the helper returns the result chosen before its receive. [Helper](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erts_internal.erl#L515-L534).

## 2. Link identity and the acknowledgment protocol

A local `ErtsILink` contains the peer PID and `Uint64 unlinking`; zero represents an active endpoint. The pending operation ID is separate from process identity and from the endpoint allocation. The generator advances the caller's `uniq` counter and skips zero. This review does not establish exhaustion/wrap safety. [Representation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.h#L1630-L1645), [ID generation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.h#L1907-L1920).

The local link BIF allocates an independent peer endpoint and sends it as LINK. Repeated active link sends nothing; a local relink while unlinking resets the ID to zero and sends a fresh peer endpoint. In contrast, **receiving** LINK while already active *or unlinking* releases the incoming endpoint without changing local state. [Sender](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L155-L183), [receiver](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6635-L6662).

| Local event | Endpoint transition | Operation ownership |
|---|---|---|
| Unlink active peer | Send UNLINK; set returned nonzero ID. Clear local priority participation. | Separately allocated record contains sender PID and ID. |
| Send cannot enqueue | Delete local endpoint immediately. | Sender destroys operation; zero denotes failure, not pending ID. |
| Receive UNLINK, active endpoint | Delete endpoint. | Always reply ACK for a process sender. |
| Receive UNLINK, own unlink pending | Preserve own inactive endpoint/ID. | Reply ACK for peer's independent operation. |
| Receive matching ACK | Delete inactive endpoint. | Destroy ACK record. |
| Receive stale ACK after relink or newer unlink | Leave endpoint unchanged. | Destroy ACK record regardless. |

[Unlink BIF](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1214-L1250), [operation producer](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L2629-L2690), [local consumers](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6698-L6771). Local navigation: [producer](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:2643), [ACK guard](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6758).

ACK reuses the received operation allocation, changes its sender PID, and preserves its ID. The receiver emits it within UNLINK handling, before later signals to that peer. Combined with same-sender signal ordering, this makes ACK a causal delimiter. The protocol documentation explicitly explains why an incoming LINK must not reactivate an outstanding unlink: that LINK preceded the peer's acknowledgment and the peer will subsequently handle the unlink. Deleting the inactive record early or accepting every incoming LINK loses this distinction. [Protocol state transitions and ordering](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/doc/guides/erl_dist_protocol.md#L1231-L1308), [language ordering](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L495-L511).

### EXIT is not an ACK with another tag

Ordinary linked EXIT is encoded as a generic exit signal; it carries no local unlink-operation ID. Reception looks up the current endpoint by peer PID. Missing or inactive means suppress the effect; an existing inactive endpoint is nevertheless deleted. Active means apply ordinary exit/trap rules, then remove the relationship. Consequently relinking can make an older, still-unhandled linked EXIT effective. “Stale ACK cannot erase the relink” does **not** imply “every pre-relink EXIT is ignored.” [Linked EXIT producer](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L2579-L2598), [reception](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4674-L4755).

Exit cleanup skips propagation through a locally unlinking endpoint. A later LINK handled by an already-exiting process gets `noproc`; an exiting process still answers UNLINK and destroys ACK, without resurrecting endpoints. This separates a new unsuccessful link from the dying process's former relationship. [Cleanup suppression](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14067-L14083), [exiting dispatcher](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7347-L7392).

## 3. Monitor cancellation follows ownership, not an acknowledgment

Ordinary local monitors share one `ErtsMonitorData` with origin and target nodes, reference identity, and initial reference count two. Each node has its own membership state. Origin points toward target; target points toward observer. The origin is installed in the observer's reference-keyed tree before MONITOR sends the target node. [Allocation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.c#L896-L914), [publication](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L779-L798).

There is an additional pre-enqueue state: a normal scheduler can retain MONITOR in `pending_signal` and return success. A following DEMONITOR to the same PID flushes that pending MONITOR **before** cancellation. If target lookup now fails, it generates DOWN/noproc for the pending monitor and reports enqueue failure; the demonitor producer releases its origin obligation. Origin permission has already been removed, so the generated DOWN cannot later materialize. A model starting with “monitor already installed on target” misses this branch. [Pending-signal ordering/failure](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L1102-L1166).

For ordinary cancellation the BIF deletes the origin before sending DEMONITOR, using **that removed origin node itself as the signal**. No target acknowledgment is required. Invalid or absent-reference branches precede mutation; explicit-un-alias preservation is a separate replacement branch. [Removal](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L437-L523), [ownership transfer](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L2930-L2967).

| Event/state | Result and release responsibility |
|---|---|
| Target handles MONITOR | Insert target node into its local-target monitor list. |
| Target handles DEMONITOR while target node remains installed | Remove target; release both origin-in-signal and target obligations. |
| Target node already detached by death | DEMONITOR releases origin only; target obligation belongs to DOWN production/delivery. |
| Target dies, immediate reason | Target node itself becomes DOWN signal. Failed enqueue releases that node. |
| Target dies, compound reason | Produce generic DOWN with copied reason/reference, then release target node. |
| Observer handles immediate DOWN | Only still-installed shared origin permits conversion. Otherwise release target signal without mailbox insertion. |
| Observer handles compound DOWN | Reference lookup must find an eligible origin; absent origin or standalone alias suppresses insertion and frees signal storage. |
| Genuine DOWN converts | Remove/replace origin and release remaining endpoint obligations; mailbox message owns its payload independently. |

[Target handling](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6544-L6629), [DOWN production](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L2814-L2893), [origin gates](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6362-L6429), [completion](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6455-L6509). Atomic reference-count decrement destroys storage only when all obligations finish; this is not proof of a Rust memory model. [Release primitives](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.h#L1503-L1530). Local navigation: [origin gate](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6362), [cancellation consumer](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6589).

The decisive race is origin removal versus **notification conversion**, not target death versus cancellation request. Death may already have produced a signal when successful cancellation suppresses its later conversion. Conversely, once converted, plain demonitor does not remove the message. Observer-local signal handling and BIF execution serialize these mutations under the process execution protocol.

## 4. Flush and the conditional boundaries

An active ordinary origin takes the successful removal path and skips the flush receive entirely. An absent origin enters `{_,Ref,_,_,_}` receive with timeout zero, removes at most one match, and returns the preselected result. It neither authenticates tuple provenance nor blacklists later application sends. Report 29 already executed the active-versus-absent synthetic-tuple discrepancy; the two-look-alike and later-send variants below remain unexecuted. Bream's typed envelope mapping remains an admission/API question, not resolved by the C optimization.

Aliases make identity insufficient: demonitor with `explicit_unalias` replaces the monitor origin with a standalone alias using the same reference. A late compound DOWN must reject that alias record rather than treat reference equality as monitor authority. The upstream GH5310 regression varies immediate/compound reasons and scheduler placement. [Regression](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/monitor_SUITE.erl#L978-L1038).

Priority changes mailbox placement, not cancellation permission or ACK ordering. Conversion inserts a priority DOWN before decrementing final monitor-priority participation; otherwise it could remove the priority queue too early. Already accepted priority messages are not retracted by capability removal. Distributed links additionally compare connection incarnations for connection-loss exits; distributed cancellation has separate distribution-list ownership and transport failure paths. Port, time-offset and persistent monitors are not interchangeable with this one-shot local protocol. [Priority completion](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6495-L6509), [connection check](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4687-L4705), [remote cancellation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L525-L589).

## 5. Additional branch cases and executable oracles

The feature index ended at LIFE-02.C14 and LIFE-03.C11 when checked. These **nine proposed cases are unimplemented and unexecuted here**, refining existing broad cases without selecting a Rust representation.

| Case | Trigger, required/forbidden observation and acceptance method |
|---|---|
| LIFE-02.C15 | MONITOR remains pending when canceled, including target disappearance. Preserve MONITOR-before-DEMONITOR; suppress resulting DOWN after origin removal; release both obligations. Model scheduler pending slot, not only receiver queue. |
| LIFE-02.C16 | In-flight immediate versus compound DOWN crosses cancellation. Origin-removal-first forbids later genuine notification; conversion-first permits retained DOWN without flush. Trace both representation/release paths and assert exactly-once destruction. |
| LIFE-02.C17 | Absent origin; two matching five-tuples and unrelated sentinel already queued. Flush removes first eligible match only, preserves remainder/order, returns false with info. Compare active-origin fast path separately; typed constructibility unresolved. |
| LIFE-02.C18 | Application sends a look-alike after cancellation/flush returns. It remains receivable; suppression applies to monitor-generated notification, not arbitrary later message. Use post-return sender handshake; forbid installing a reference blacklist. |
| LIFE-03.C12 | ACK for unlink reaches a reactivated endpoint. Preserve the active relation. Use ordered peer barrier and subsequent linked death; instrument interleaving to prove ACK was pending at relink. |
| LIFE-03.C13 | Unlink i → relink → unlink j; ACK i arrives while j is pending. Old ACK must not erase j; ACK j can finish cleanup. Inject this exact sequence in protocol model and verify allocation accounting. |
| LIFE-03.C14 | Incoming LINK while own unlink remains pending, contrasted with own relink call. Incoming LINK cannot reactivate; own relink can. Enumerate crossed operations and drain ACKs before checking endpoint agreement. |
| LIFE-03.C15 | Linked EXIT meets inactive endpoint, with ACK still pending. Suppress notification/failure and remove endpoint; later ACK is harmless. Separate already-materialized EXIT and reactivated endpoint controls; no unconditional stale-EXIT filter. |
| LIFE-03.C16 | Peer begins exit with unlink pending; late incoming LINK arrives. Do not propagate historical reason through inactive old link; late LINK follows noproc handling. Qualify alive-check races and exercise both exiting dispatch and absent lookup. |

Two public-API probes provide useful deterministic observations. First, an isolated trapping caller links, unlinks and relinks a waiting peer, then sends a barrier request. The peer replies with its link list and waits for an exit command. Its ordinary reply follows UNLINK_ACK; check both endpoints remain linked, then require linked EXIT and independent monitor DOWN. This proves survival after acknowledgment drainage, **not** that scheduling forced the stale-ACK branch.

Second, install canceled monitor R and surviving monitor S on a peer that cannot exit before a command. Remove R with info=true, permit death, receive DOWN(S), then reject DOWN(R). A separate cohort waits until inspection confirms genuine DOWN(R) is materialized without consuming it: plain demonitor/info returns false and retains it; subsequent flush/info returns false and removes it. These controlled observations distinguish mailbox history from future suppression; forcing an already-produced but unconverted DOWN needs internal scheduling control.

Selected upstream tests provide broader oracles, not executed coverage: the concurrent link fixture uses a ping/pong/pling barrier before checking endpoint consistency; the unlink/exit regression explicitly admits its rare branch is difficult to trigger; the monitor-removal fixture distinguishes true/no-DOWN from false/queued-DOWN under a first-call/no-consumer setup. [Link barrier](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/erl_link_SUITE.erl#L549-L611), [rare exit case](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/signal_SUITE.erl#L764-L857), [monitor oracle](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/test/monitor_SUITE.erl#L363-L394).

**Remaining gaps:** complete interleaving exploration, operation-ID wrap, allocator failure injection, C memory-order proof, complete remote/priority/alias products and Bream's Rust lifetime representation. The useful implementation obligation is already clear: retain enough state to distinguish active permission, pending protocol work, accepted mailbox content and outstanding allocation ownership.

Plan integration: these IDs are recorded in the [build plan](../build-plan.md). Source evidence does not qualify the Bream implementation.
