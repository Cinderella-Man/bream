# Session replacement and stale incarnation isolation

**Twelfth bounded milestone passed for the declared single-worker profile, 2026-10-08.** This experiment composes the existing checked process, mailbox, monitor and immutable-byte contracts into one room member replacement. The runtime and compiler are inherited from [busy-session](../busy-session/README.md), with only the experiment's crate names changed. It extends partial G5 and does not complete the full build gates. [Milestone tracker](../../docs/milestones.md).

**No speed tests or performance comparisons may run on this machine.** The controls use protocol stages, identities, checksums, ownership counters and cleanup observations. Watchdogs only detect hangs. Allowances and elapsed time do not establish a production setting, throughput improvement or latency guarantee. Performance qualification requires another suitable environment and explicit authorization. [Persistent instructions](../../AGENTS.md).

## Bounded replacement protocol

The [checked application and host controls](consumer/src/replacement.rs) use one worker and a registry with exactly three slots. A room creates two monitored members: the replaceable member at application epoch 1 and a surviving member at epoch 9. Each member also monitors the room. These three live processes fill the registry. When the old member exits, it leaves the only free slot; successful creation of the replacement therefore forces reuse of that slot. The room explicitly requires a different full process identity and a different monitor reference for the replacement, whose application epoch is 2.

The trusted host ingests immutable bytes `[2, 3, 5, 7]`. The host, room and two initial members independently own a share. A member moves its handle and owned reply buffer into an acyclic helper, computes checksum 62627, writes its epoch and checksum into the buffer, sends the owned reply and returns its handle. Helper calls move ownership without cloning a shared reference. Explicit broadcast and stale-send operations are the only application paths that acquire new shares in this fixture.

The room first validates both startup replies and one request/reply exchange with the surviving member. The old member then sends a real final report carrying epoch 1, sequence 99 and the payload checksum. It subsequently either returns normally or fails through an admitted out-of-bounds byte read. The failure case exercises the existing contained process-failure path; it does not add a checked panic primitive.

The room waits selectively for replacement input. The host requires both the old report and the genuine runtime `Down` to be queued and scanned past before sending that input. It also queues a trusted request bearing the retired epoch. After replacement, a selective readiness receive skips those three old events. The room installs the new member's identity and monitor, checks its checksum reply, cancels the retired monitor reference through plain `demonitor`, and sends both an owned word buffer and a shared handle to the old PID. Those sends must consume their ownership without delivering to the replacement. Plain cancellation must leave the already queued old `Down` intact and must not cancel the replacement's monitor.

This is **deferred application consumption of an already published old `Down`**. It does not simulate delayed runtime publication after slot reuse. The old reply is also a real message emitted before the old member exits, not a host-authored substitute.

## Identity and application epochs

Runtime `Down` carries a sealed monitor reference, full target PID and reason. The room accepts the retired notification only when all three match the retired member. It accounts for that retired record without replacing or deleting current membership. At normal close it separately requires the replacement's own exact monitor reference, full PID and normal reason.

Ordinary `Words` envelopes have no authenticated sender identity. The scalar epoch belongs to this trusted application protocol: it distinguishes old requests from current membership but is not a security boundary or a substitute for runtime identity. The rejection case checks that the queued request's epoch differs from the current epoch and consumes its buffer without routing it. General sender authentication, reply capabilities and request deduplication are outside this milestone.

The fixture's message order comes from explicit protocol gates and same-sender ordering. It does not assume a total order among room, old member, replacement, survivor and host. There is no priority-message support in this profile.

## Correctness and ownership oracles

Four scenarios run for each configured compiler profile and allowance:

| Scenario | Required observations |
|---|---|
| Replacement after normal exit | The old report and exact normal `Down` survive replacement readiness and retired-monitor cancellation. Both are consumed once; the stale epoch request is rejected; current and surviving members answer subsequent checksum probes. |
| Replacement after process failure | The same sequence requires the old member's exact failure `Down`, then verifies replacement and survivor progress. |
| Explicit room kill after replacement | The room stops while three old events and nineteen extra shared envelopes remain queued. Both live members observe their room monitor and terminate. All held and queued ownership must be released. |
| Active runtime shutdown after replacement | Shutdown begins at the same retained-event boundary and settles all processes, relationships, queues and logical timers. |

For continuing scenarios, the room records observed epoch/sequence/checksum results before replacement, after consuming stale events and after another survivor exchange. It counts one old report, one matching old `Down` and one rejected old request. The replacement's ordinary and shared stale-PID sends are checked through released ownership and subsequent successful operation of the replacement. An unexpected delivery to a member makes its checked protocol fail or prevents the required reply. Final close requires a member acknowledgement and its matching normal `Down`; acknowledgement payload contents are not a separate oracle.

At the initial settled boundary there is one four-byte backing allocation and four references. Old-member retirement leaves three. Replacement acquires another reference and the deliberately stale shared send acquires then releases one: the settled state again has four live references, with six created and two released. These counters also check that intervening helper ownership moves do not clone the payload.

In termination scenarios the room retains three old events and nineteen shared envelopes, totaling twenty-two queued events. There are then twenty-three live references, with twenty-five created and two released. Room cleanup must retire the twenty-two events in three batches under the inherited eight-event allowance. Genuine monitor cleanup and process-frame destruction settle separately from queue ownership.

After all process cleanup, only the host handle may remain. The host reads all four bytes, then releases the final reference. The receipt must report zero live bytes and references, matching created/released reference counts, and exactly one physical backing-storage destruction. Every tracked ordinary message buffer must also be released exactly once. Shared counters describe vector capacity and reference obligations, not RSS, allocator retention or all runtime allocations. Their independently loaded fields are compared at established ownership boundaries. Checksums are bounded content oracles, not proofs of arbitrary payload equality.

## Qualification

The [passing cohort](../../evidence/session-replacement/runs/20261008-replacement-01/results.json) records 387 expected command outcomes, 352 observations and 187 preserved inputs. It includes six replacement configurations: allowances 1, 4 and 32 under debug and release, giving twenty-four new scenarios. Each compiler profile retains the inherited 95 runtime unit tests, 148 rejected checked consumers and eight positive shadowing controls, alongside prior public control families. The [read-only audit](../../evidence/session-replacement/validation.json) checks the retained results, current inputs, prior busy-session evidence and eighteen runtime/compiler files after crate-name normalization. [Evidence guide](../../evidence/session-replacement/README.md).

No runtime API, compiler admission rule or scheduler behavior is added here. The new fixture investigates how the existing contracts compose. Prior experiment sources and evidence remain separate.

## Pinned BEAM source obligations

The inspected local OTP checkout is pinned to `ad05823719d77c8faee87348ea39513d4e2f99c5`. This review inspected source and documentation and did not execute BEAM.

| Source branch | Implication for this Rust fixture |
|---|---|
| [Full-PID process lookup](../../upstream/otp/erts/emulator/beam/erl_process_lock.h:1135), [PID index versus identity data](../../upstream/otp/erts/emulator/beam/erl_node_container_utils.h:128) | BEAM indexes the process table then rejects a mismatching full PID. A reused slot is not enough to select a new incarnation. Bream preserves runtime namespace, slot and generation in its sealed identity. |
| [Process-table slot release](../../upstream/otp/erts/emulator/beam/erl_ptab.c:694) | OTP clears the slot and advances its identity data before reuse. Its masked data can eventually wrap; do not describe OTP PIDs as universally never reused. Bream deliberately checks generation exhaustion and retires an exhausted slot rather than wrapping. |
| [Ordinary PID send](../../upstream/otp/erts/emulator/beam/bif.c:2470), [nonexistent-PID send contract](../../upstream/otp/system/doc/reference_manual/expressions.md:659) | An absent full identity discards a send rather than retargeting another process. Rust must still consume the moved buffer or handle and settle its ownership correctly. |
| [Fresh monitor creation](../../upstream/otp/erts/emulator/beam/bif.c:758), [independent monitors and Down tuple](../../upstream/otp/system/doc/reference_manual/ref_man_processes.md:709) | Target PID and monitor reference identify different obligations. Repeated monitoring creates independent references; replacement cannot inherit an old monitor by occupying its slot. |
| [Down from the exiting incarnation](../../upstream/otp/erts/emulator/beam/erl_process.c:13798), [stored reference and target](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:4940), [Down message construction](../../upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:5034) | Runtime notification preserves the old target and reference. Room membership changes must compare them with the relevant incarnation instead of treating any Down as current-member death. |
| [Selective receive semantics](../../upstream/otp/system/doc/reference_manual/expressions.md:680), [nonmatching-message scan](../../upstream/otp/erts/emulator/beam/emu/msg_instrs.tab:249), [selected-message removal](../../upstream/otp/erts/emulator/beam/emu/msg_instrs.tab:227) | Matching a new readiness message can leave old replies and Down untouched. Replacement must safely handle their later application consumption. |
| [Signal-order guarantee](../../upstream/otp/system/doc/reference_manual/ref_man_processes.md:497), [ordinary and priority queue ordering](../../upstream/otp/system/doc/reference_manual/ref_man_processes.md:345) | Ordering is sender-to-destination, not global. OTP priority-message reception introduces additional queue-order rules; this candidate does not implement it. |
| [Visible-resource guarantee and exclusions](../../upstream/otp/system/doc/reference_manual/ref_man_processes.md:462), [heap-held resource lifetime](../../upstream/otp/system/doc/reference_manual/ref_man_processes.md:489) | Down does not imply universal physical-memory reclamation. Shared bytes may correctly survive a retired member through the room, replacement, survivor or host reference. |
| [Fresh-reference receive optimization](../../upstream/otp/system/doc/efficiency_guide/eff_guide_processes.md:154), [receive marker instructions](../../upstream/otp/erts/emulator/beam/emu/msg_instrs.tab:24) | A future Rust receive marker could avoid inspecting messages predating a fresh sealed token. A trusted scalar epoch alone cannot establish that proof; all matching arms would need to require the fresh token. This is a source-derived opportunity, not measured performance evidence. |

## Remaining scope

This is one bounded replacement in a fixed two-member room. It does not qualify arbitrary membership collections, repeated restart supervision, restart limits, authenticated application senders, durable delivery, general reply correlation, multiple workers, unrestricted native Rust, real-time timer driving or full BEAM conformance. Further source and correctness work must preserve response ordering, generation exhaustion, bounded scan/cancellation work and shared ownership. All speed qualification remains outstanding in a separately authorized suitable environment.
