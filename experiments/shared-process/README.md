# Immutable byte sharing and bounded broadcast

**Tenth bounded milestone passed for the declared single-worker profile, 2026-10-08.** This experiment extends the [room/session profile](../room-process/README.md) with runtime-managed immutable bytes and explicit sharing across process lifetimes. It implements a restricted part of the [accepted immutable-sharing decision](../../docs/adr/0008-immutable-buffer-sharing.md). The [retained qualification](../../evidence/shared-process/README.md) passes 375 expected command outcomes, including 95 unit tests per compiler profile, six sharing configurations, 296 expected compiler rejections and sixteen positive shadowing executions. This does not complete G5. [Milestone tracker](../../docs/milestones.md).

**No speed tests or performance comparisons may run on this machine.** Source inspection, builds and bounded correctness controls are the available evidence. Logical checkpoints, injected clock ticks, ownership counters and allocation identity establish semantic properties only. Watchdog timeouts detect hangs; their elapsed values are not performance evidence. Performance qualification requires a separately authorized suitable environment. [Persistent instructions](../../AGENTS.md).

## Byte storage and ownership

The trusted host creates `SharedBytes` with `SharedBytes::new(Vec<u8>)`, receiving an immutable handle and a `SharedReceipt`. The constructor consumes the vector and rejects a capacity greater than 65,536 bytes, including a short vector with excessive spare capacity. The checked process language cannot invoke this constructor, allocate arbitrary byte vectors, freeze mutable state or introduce its own shared types. Host ingestion is the deliberate boundary for this slice.

The [storage implementation](runtime/src/shared.rs) puts the owned `Vec<u8>` inside a private `Storage` held through `Arc`. Moving the vector into this structure preserves its existing byte allocation; it does not copy the bytes or require shrinking spare capacity. An explicit handle clone acquires another reference to that same storage. This is a source and allocation-identity claim, not a measured speed improvement. The wrapper and accounting still allocate, reference updates still perform atomic work, and each accepted message still needs queue storage.

The byte API exposes length and indexed reads. It exposes no mutable borrow, mutable pointer, slice mutation, thaw operation, replacement of the backing vector or raw `Arc`. Indexed reads outside the current length follow Rust's bounds failure path. The admitted value is this concrete runtime-owned byte type, not arbitrary `Arc<T>`, destructor-bearing shared resources or shared mutable application state. Trusted host Rust code remains outside the checked process-language guarantee.

`SharedEnvelope` owns a numeric tag and one `SharedBytes` handle. Sending consumes the handle; the sender must explicitly clone first if it needs its own reference or another recipient needs one. Rejected delivery consumes and drops that message's handle. Successful delivery transfers the obligation into the recipient's mailbox, and a receive transfers it into the recipient's continuation. A publisher stopping releases its own references while live recipients keep the bytes. A recipient stopping releases its references while other owners keep theirs. Only the final payload reference releases the backing allocation.

`SharedReceipt` owns accounting counters only. Keeping or cloning a receipt cannot keep the payload alive. Its snapshot separates the following quantities:

| Snapshot fields | Meaning |
|---|---|
| `live_bytes`, `freed_bytes` | The original vector's backing capacity, counted once for this storage, then recorded as released after the vector is dropped. These are not per-recipient charges or RSS. |
| `physical_drops` | Destruction of this backing storage, exactly once after its last payload handle disappears. A zero-capacity vector still has one storage destruction. |
| `live_references`, `references_created`, `references_dropped` | Payload-handle obligations. Clone creates an obligation; moving a handle does not. |

The counters exclude `Arc` headers, envelope nodes, accounting allocations and allocator retention. Snapshot fields are independent atomic loads, not one atomic multi-field observation. Exact cross-field balance assertions therefore require an established quiescent ownership boundary, such as completed cleanup with other holders parked. A process cleanup receipt does not claim that bytes retained by another live process or the host have been freed.

## Checked operations and one event queue

The [checked compiler](macros/src/structured.rs) admits concrete `SharedBytes`, `SharedMessage` and `SharedReceiveResult` kinds. `SharedMessage` lowers to the runtime's `SharedEnvelope`; authored code accesses it through admitted operations rather than its private fields.

| Authored operation | Ownership and selection contract |
|---|---|
| `clone_shared(bytes)` | Borrow a concrete shared-byte place and return a new owned handle. |
| `shared_len(bytes)`, `shared_read(bytes, index)` | Read the immutable payload through a concrete shared-byte place. |
| `send_shared(pid, tag, bytes)` | Evaluate arguments and move one handle into an ordinary local shared-payload message. |
| `receive_shared(tag)`, `receive_shared_any()` | Select the first matching shared-payload message; preserve ordinary messages and notifications. |
| `receive_shared_for(tag, ticks)`, `receive_shared_any_for(ticks)` | Use the inherited injected-clock receive timer and return `SharedReceiveResult::Message` or `Timeout`. |
| `shared_tag(message)`, `shared_bytes(message)` | Read the tag or consume the envelope to recover its owned payload handle. |
| `receive_event()`, `receive_event_for(ticks)` | Select from the common stream, now including `Event::Shared` beside `Message`, `Down` and `Exit`. |

There is one mailbox order, not a separate shared-byte inbox. Ordinary `receive_any()` continues to select ordinary `Words` messages only. Shared selectors preserve unmatched ordinary messages, runtime notifications and other shared tags. The inherited saved-prefix scan, generated checkpoints and timer cancellation rules apply to this new selector as well.

Queued shared envelopes join the existing retirement path: each cleanup step releases at most eight queued events, and each shared envelope owns one payload reference. This is a bound on queued release operations, not on elapsed deallocation time or every reference in a generated continuation. The final `Vec<u8>` release can enter the allocator. No arbitrary user destructor is introduced by this byte-only representation.

## Bounded qualification scenarios

The application fixture has two fixed monitored session children and zero, one or two payload recipients. A session outside the recipient count returns without receiving a payload. The publisher explicitly clones one handle per send, collects checksum acknowledgements and genuine Down notifications through the common event stream, and checks the child's full process identity and monitor reference. Zero recipients must create no recipient payload obligations. A child's checksum acknowledgement must precede that child's Down under the fixture's ordinary same-sender ordering. No total order between different children is assumed.

Separate fixtures with known process handles hold immutable readers at explicit protocol barriers. They require readable byte contents after publisher kill and after another receiver's kill, and require the remaining holder's reference to keep exactly one backing allocation live. A host-held final reference must retain the payload after all process cleanup and release it exactly once when dropped. Partial fanout cancellation checks reference accounting: stopping the publisher between sends must leave the first published reference live and must not create the second. It does not inspect that child’s post-stop checksum; the separate known-handle publisher-death fixture checks readable survival. Another stop inside a suspended cloned send operand must release both sender references before any message is published. Queued shutdown must release every queued reference even when no application receive consumes it.

Deterministic unit controls cover rejected delivery, mixed-event ordering, shared selection and timeout preservation, capacity admission, storage identity and cleanup batches. Public checked-program controls cover the broadcast and lifetime scenarios, plus sixteen shared-send/kill races per configuration. Compilation rejection cases keep raw shared types, mutation, direct methods, forged runtime values and invalid ownership outside the admitted boundary; positive controls check generated-name hygiene and argument evaluation.

The [passing cohort](../../evidence/shared-process/runs/20261008-shared-01/results.json) retains the protocol, 185 inputs, exact commands, raw outputs, source hashes and 340 structured observations. Six sharing configurations cover ten lifetime/broadcast scenarios each and 96 shared-send/kill races. The runtime suite adds twelve ownership and mailbox tests to the previous 83. The [read-only audit](../../evidence/shared-process/validation.json) checks these records and preserves the prior room cohort. A bounded sample of interleavings is not an exhaustive concurrency proof. Checksum observations are bounded content oracles, not collision-free byte equality proofs.

## Pinned BEAM obligations

The inspected local OTP source is pinned to `ad05823719d77c8faee87348ea39513d4e2f99c5`. This source review did not execute BEAM. The efficiency guide explicitly says its representation description predates the OTP 27 rewrite, so the implementation anchors below use `ErlHeapBits`, `ErlSubBits`, `BinRef` and `Binary`. [Documentation caveat](../../upstream/otp/system/doc/efficiency_guide/binaryhandling.md:119).

| Source branch | Obligation or limitation for Bream |
|---|---|
| [Heap threshold](../../upstream/otp/erts/emulator/beam/erl_bits.h:166), [new bitstring allocation](../../upstream/otp/erts/emulator/beam/erl_bits.c:2757) | BEAM normally creates bitstrings of at most 64 bytes inline on a heap and larger ones with separate backing storage. Its threshold is an implementation choice, not Bream's chosen sharing threshold. |
| [Reference-backed message copy](../../upstream/otp/erts/emulator/beam/copy.c:812) | Copying a `BinRef`-backed value creates destination descriptors, pins writable storage, increments the backing reference and attaches recipient ownership. Sharing payload bytes does not eliminate per-recipient work. |
| [Heap-backed match context](../../upstream/otp/erts/emulator/beam/copy.c:849), [ordinary send](../../upstream/otp/erts/emulator/beam/erl_message.c:821) | Some bytes and ordinary term structures are copied. Bream must not describe all BEAM messages or binaries as copy-free. |
| [Writable backing pin](../../upstream/otp/erts/emulator/beam/erl_bits.c:2051) | First sharing can clear write permissions and shrink via reallocation. Bream's host-owned immutable ingestion avoids a mutable alias and compulsory shrink, while retaining spare capacity explicitly. |
| [Exiting-recipient rejection](../../upstream/otp/erts/emulator/beam/erl_message.c:413), [message cleanup](../../upstream/otp/erts/emulator/beam/erl_message.c:229) | A rejected or discarded message must release its acquired payload obligations. |
| [Receive adoption](../../upstream/otp/erts/emulator/beam/erl_message.c:965) | Receiving transfers fragment references into process ownership; removing a queue node does not imply that retained bytes are freed. |
| [Process cleanup](../../upstream/otp/erts/emulator/beam/erl_process.c:13405), [off-heap release](../../upstream/otp/erts/emulator/beam/erl_message.c:165), [final reference release](../../upstream/otp/erts/emulator/beam/erl_binary.h:389) | Each lifetime releases its own references. The final backing reference controls byte reclamation, independently of publisher identity. |
| [Ordinary versus magic binary free](../../upstream/otp/erts/emulator/beam/erl_binary.h:373) | Arbitrary native destructor callbacks have a different contract from plain immutable byte storage and are not admitted by this slice. |
| [Backing-size accounting](../../upstream/otp/erts/emulator/beam/erl_bits.h:346), [referenced byte size](../../upstream/otp/erts/emulator/beam/erl_bif_binary.c:2520) | Visible length, per-owner accounting and uniquely retained backing storage are different quantities. Small views can retain large allocations; Bream currently exposes no slicing API. |
| [Yielding relationship cleanup](../../upstream/otp/erts/emulator/beam/erl_process.c:14545), [final queue cleanup loop](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:9589) | BEAM's exit work contains both yielding phases and final linear release loops. Bream's explicit queued-event release budget is a structural choice, not evidence of lower cleanup latency. |
| [Signal ordering](../../upstream/otp/system/doc/reference_manual/ref_man_processes.md:507), [selective receive](../../upstream/otp/system/doc/reference_manual/expressions.md:680) | Ordinary messages from one sender preserve order; selective receive consumes the first match and preserves unmatched messages. A shared-byte representation must not change these semantics. |

The fuller [message-allocation source review](../../docs/research/34-message-allocation-and-copy-costs.md) covers first-sharing, append-after-sharing, many-view fanout and retained-slice pitfalls. Those branches inform the boundary without admitting Erlang bit syntax or every binary operation into this experiment.

## Scope still open

This remains a bounded single-worker implementation profile. It does not qualify arbitrary shared Rust types, mutable sharing, slices, authored byte construction or compaction, large or dynamic membership, a session that itself hogs computation, broader join/fanout/death interleavings, real-time timers, multiple workers or full BEAM conformance. It does not add networking, restart policy or a server framework.

The inherited room/session conversation and this fixed broadcast cover different application obligations; neither closes the entire G5 gate. Further application milestones must address the remaining membership and cancellation work, while speed qualification remains deferred to an explicitly authorized suitable environment.
