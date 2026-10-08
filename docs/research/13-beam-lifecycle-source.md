# BEAM lifecycle source: identity, relationships, and termination

Research snapshot: 2026-10-07. Read-only source review of Erlang/OTP **29.1.1**, commit `ad05823719d77c8faee87348ea39513d4e2f99c5` (2026-09-21), checked out at `/home/bat/projects/bream/upstream/otp`. All source links below pin that commit. The [official online process reference](https://www.erlang.org/doc/system/ref_man_processes.html) also identified itself as 29.1.1 when checked.

This report traces ordinary **node-local process** creation, links, monitors, exit signals, and exit cleanup. It does not audit GenServer, supervisors, distribution, priority messages, process aliases, ports, or every monitor kind. Those paths share machinery, but reading adjacent branches is not sufficient to claim coverage. No OTP build, concurrency test, or latency measurement was run.

**Finding:** the reusable idea is a runtime-owned lifecycle protocol: publish a distinct incarnation; establish initial relationships before execution; distinguish incoming control signals from application messages; stop user execution irreversibly; finish resource and relationship obligations through scheduled cleanup; reclaim the remaining execution storage when safe. None of this justifies destroying an arbitrary suspended Rust stack.

The accepted Bream requirements remain in [the design tree](../design-tree.md), including BEAM-style exits/trapping, safe forced termination with runtime-managed cleanup, and investigation of a Cargo-generated supported process model. During this review the user also selected one runtime envelope for application messages and lifecycle notifications, runtime-managed immutable bytes with shared references, and DOWN after execution has stopped and identity has been removed, with tracked cleanup completing separately. Exact API syntax remains open. Recommendations here are proposals, not additional user decisions.

## 1. Creation publishes an incarnation, not a service

`alloc_process` allocates the process record and asks `erts_ptab_new_element` to install it. Its early-init callback assigns the PID, initializes scheduling state, and initializes all process locks in the locked state. In the current free-ID-table path, the table allocator runs this callback before release-publishing the record pointer. Later initialization occurs while the creation path owns the process locks. Thus “the pointer is in the table” and “the process may run application code” are different events. The deprecated allocation branch is also present; this is not an audit of every configuration.

Sources: [process allocation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12159-L12218); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:12159); [table publication](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L500-L560); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_ptab.c:500).

The new process starts with empty internal and incoming signal queues, zero mailbox length, no process dictionary, and no BIF timers. Ordinary creation does not transfer a dead process's queued messages or identity. Application arguments are a separate part of spawn setup, not continuation of a previous incarnation's mailbox.

Source: [fresh queues and process-local state](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12722-L12749); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:12722).

### Stale identities

The raw lookup indexes the process table using the PID's index, then compares the **complete PID** against the record's current ID. A slot match alone is insufficient. The convenience lookup also rejects a record already marked exiting. Raw lookup has a thread-progress lifetime precondition; a correct generation check alone would not protect a borrowed native record from concurrent reclamation.

Source: [raw and live process lookup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process_lock.h#L1135-L1156); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process_lock.h:1135).

Deleting a table element clears the slot and, in the free-ID-data path, advances the ID data by the table size before returning it for future allocation. The value is masked to a finite width. This is a changing-incarnation identity, not a promise that an ancient PID can never recur. Erlang explicitly permits eventual reuse of a terminated PID. The spawn-link implementation even accounts for a rare collision associated with PID wrap.

Sources: [table deletion and next incarnation data](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_ptab.c#L675-L735); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_ptab.c:675); [documented PID reuse](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/data_types.md#L310-L315); [local](/home/bat/projects/bream/upstream/otp/system/doc/reference_manual/data_types.md:310); [spawn link collision handling](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12911-L12926); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:12911).

**Bream implication:** keep runtime instance, process incarnation, and typed message address conceptually separate. Any slot-table implementation needs full-identity validation and a record-lifetime protocol. Decide explicitly whether finite-generation exhaustion fails allocation, permanently retires slots, or permits documented eventual reuse. A PID check must not be advertised as a liveness guarantee after the check returns.

## 2. Atomic spawn-link and spawn-monitor close a real race

`spawn_link_3` supplies `SPO_LINK` to `erl_create_process`. The Erlang `spawn_monitor` wrapper routes to `spawn_opt(..., [monitor])`; it is not implemented as a user-level spawn followed by a separate monitor call.

Sources: [spawn_link BIF](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1003-L1022); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:1003); [spawn_monitor wrapper](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L10657-L10670); [local](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erlang.erl:10657).

During node-local creation, before scheduling the child:

1. The link option installs the parent-side and child-side link entries.
2. The monitor option creates paired monitor endpoints, inserts the origin in the parent's monitor tree, and inserts the target in the child's local-target-monitor list.
3. Only afterward does the creation path call `schedule_process`.

Sources: [initial relationship installation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12908-L12945); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:12908); [first scheduling](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13115-L13121); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:13115).

This closes the interval in which a child could execute and die before its initial link or monitor exists. It does **not** promise that the child is still alive when the parent resumes, nor that the child's application initialization succeeded. For asynchronous `spawn_request`, there is an additional explicit ordering step: send the success reply before any child DOWN or linked exit can reach the parent.

Source: [spawn reply ordering](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12947-L12975); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:12947).

**Bream implication:** provide atomic initial relationship installation in the core spawn machinery, even if the eventual API expresses it through options rather than several functions. A readiness handshake is an application protocol above this guarantee. The first adversarial spawn test should be a child whose first action is to exit.

## 3. Monitors are paired, referenced, asynchronous relationships

For an ordinary local target, `monitor` allocates a fresh reference and paired monitor data, installs the observer's origin entry first, then sends the target-side monitor signal. If sending fails because the target is absent, it arranges a DOWN with reason `noproc`. Monitoring self is a special no-relationship case in this BIF.

Sources: [monitor origin before target signal](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L758-L801); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:758); [paired monitor data](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_monitor_link.c#L856-L915); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_monitor_link.c:856).

The signal transport makes the asynchronous nature concrete: a normal scheduler may temporarily hold the first local monitor signal in its pending-signal slot and return success. A subsequent same-target signal flushes that monitor ahead of itself to preserve ordering. Consequently, “monitor returned a reference” is not evidence that the target has already installed its endpoint.

Source: [pending monitor and ordered flush](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L1102-L1166); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:1102).

Ordinary signal reception inserts the target endpoint. If the target has already entered the exit-cleanup path when an unprocessed monitor signal is drained, that path replies with `noproc`. A monitor established before the target's exit can carry its actual reason; a late monitor need not reconstruct historical reasons. The generation/lookup checks and enqueue rejection of FREE records help route arrivals toward either the living record or the absent-target handling, rather than a recycled slot.

Sources: [install target endpoint](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6544-L6559); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6544); [enqueue/free-state handling](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L1211-L1244); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:1211); [late monitor during exit](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7347-L7353); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:7347).

### Demonitor suppresses future conversion; flushing handles an existing message

For a normal process monitor, `demonitor` removes the observer's origin entry and sends an asynchronous demonitor signal to the target. On receipt of a local DOWN signal, the observer converts it into a mailbox message only if the corresponding origin endpoint is still installed. If the origin is gone, it removes the signal and releases its monitor ownership. If conversion happens, ordinary one-shot monitor handling removes the origin and releases the pair.

Sources: [demonitor origin removal](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L436-L523); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:436); [DOWN conversion eligibility](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6355-L6379); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6355); [discard or consume the relationship](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6455-L6509); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6455).

A DOWN already converted into a mailbox message is a different state. The `flush` option uses an Erlang helper to remove a matching reference-bearing notification with a zero-timeout receive when needed. `info` reports whether the monitor entry was still removable; it is not a fresh proof that the target is alive. In the ordinary one-shot case, the live origin and an already-converted DOWN do not coexist, explaining why successful removal does not require an unconditional mailbox scan.

Sources: [demonitor options](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L613-L654); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:613); [flush helper](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erts_internal.erl#L515-L534); [local](/home/bat/projects/bream/upstream/otp/erts/preloaded/src/erts_internal.erl:515).

**Bream implication:** the monitor reference identifies one observation relationship, not just a watched PID. Model at least installed, notification-in-flight, notification-materialized, and cancelled states, with ownership accounted for on each transition. Distinguish removing a monitor from removing an already-visible event. Do not implement either operation as “delete a hash-map key and hope the other endpoint notices.”

## 4. Links need an unlink protocol, including stale acknowledgment handling

A local `link` creates or finds the caller's entry and sends the peer endpoint as a link signal. An already-active link is idempotent; re-linking an entry whose unlink is pending resets its local unlinking marker. There is an important absent-peer exception: when a local target is already absent at the early lookup and the caller does not trap exits, `link/1` raises a catchable error-class `noproc` exception immediately. A trapping caller, or failure discovered later during asynchronous link setup, instead follows the linked `noproc` exit-signal path. An error exception and an exit signal are distinct observations; this boundary must not be simplified to unconditional asynchronous failure. [Documented exception](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L3866-L3879), [early lookup and signal path](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L155-L183).

Source: [local link setup and relink](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L155-L185); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:155).

`unlink` sends an unlink operation carrying a fresh ID and records that ID in the local entry. It returns without waiting for the peer's acknowledgment. The peer removes its active link endpoint and sends an acknowledgment; the origin removes its remaining entry **only if its current unlinking ID equals the acknowledged ID**. That comparison prevents an old acknowledgment from deleting an entry restored by a newer link operation.

Sources: [unlink marks local state](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1223-L1249); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:1223); [unlink operation ID](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L2629-L2670); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:2629); [peer unlink and acknowledgment](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6698-L6716); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6698); [correlated acknowledgment](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6758-L6771); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6758).

When a linked exit arrives, reception checks whether the local link exists and remains active. An absent or unlinking entry causes that linked signal to be ignored. This is how local deactivation suppresses a linked exit still in flight. It does not flush a trapped EXIT that was already converted into an application-visible mailbox message before unlink ran.

Source: [active-link check on linked exit](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4674-L4720); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:4674).

**Bream implication:** relationship state and operation identity matter even on one machine. A single shared locked relation table could implement the same observable contract differently; Bream need not reproduce ERTS's two-endpoint layout. It still needs a demonstrable ordering rule for unlink racing with death, relink, and already-delivered notifications. The source checks above are evidence for that protocol, not a formal proof of all crossing-signal interleavings.

## 5. Forced kill is distinguished by signal operation, not the reason alone

`handle_exit_signal` first validates a linked signal against the current relationship. It then applies the trap/normal/kill rules. The crucial test combines the signal opcode with its reason: only a **direct exit signal with reason `kill`** bypasses exit trapping and becomes termination reason `killed`.

Source: [exit classification and transition](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L4738-L4783); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:4738).

For an active link or an ordinary direct signal from another process, the relevant cases are:

| Incoming control signal | Not trapping exits | Trapping exits |
|---|---|---|
| Normal reason | Ignored | EXIT message |
| Other abnormal reason | Termination with that reason | EXIT message |
| Direct `kill` | Termination with `killed` | Termination with `killed` |
| Link-propagated reason literally `kill` | Termination with `kill` | EXIT message carrying `kill` |
| Linked signal whose local link is inactive | Ignored | Ignored |

Thus a process killed directly propagates its resulting `killed` reason through links; a trapping peer can receive that as a normal trapped notification. A reason value called `kill` is not itself an unforgeable forced-termination command. The operation and relationship context supply the semantics.

On termination, `erts_set_self_exiting` stores the reason, clears suspended/dirty-work scheduling flags, sets EXITING and ACTIVE, removes the Erlang catch chain, and changes the next instruction to `beam_exit`. The normal signal-dispatch path then stops processing ordinary execution after the exit transition. Cleanup can run even for a process previously suspended.

Sources: [irreversible runtime exit state](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13485-L13547); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:13485); [stop after signal-triggered exit](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L6338-L6351); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:6338).

The sender's ordinary local exit-signal operation prepares a return value without waiting for the target's cleanup. There is a historical exception for `exit/2` directed at self: an untrapped normal reason can terminate self, and the call waits by trapping to the exit path. OTP 29's `exit_signal/2` uses the common helper without that legacy `exit2` mode. Following the later Q21 instruction, a Bream API claiming either operation's semantics should preserve that operation's actual behavior; exact Rust API coverage/names remain experimental.

Sources: [send versus legacy self-exit](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1750-L1772); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:1750); [exit and exit_signal front doors](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/bif.c#L1872-L1889); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/bif.c:1872).

**Bream implication:** keep forced kill in runtime control state, outside the typed application protocol and outside any catchable application callback. For a generated process model, the engine must transition at a qualified checkpoint and never poll/resume the user continuation afterward. Every permitted helper and native operation must be covered by this guarantee or by a documented supported boundary. The BEAM source provides no justification for injecting a Rust panic at an arbitrary instruction or dropping an active native stack.

## 6. Exiting processes execute a scheduled cleanup state machine

`erts_continue_exit_process` is a continuation with an explicit phase, saved relationship roots, exit reason, and phase-specific yield state. It begins with the remaining reduction budget. Among the phases are timer cleanup, ETS cleanup, system tasks, removal from the live table, links, monitors, remaining process signals, distribution work, and completion.

Source: [exit phases, stored continuation, and timer budget](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14234-L14315); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14234).

Timer cancellation and ETS exit processing can return to the continuation. Relationship trees and lists are traversed with yielding delete functions. Remaining process-signal cleanup has its own reduction accounting. When work exhausts its allowance, the code preserves the continuation, installs `beam_continue_exit`, and consumes the current reduction allowance. The x86 JIT's continue-exit instruction calls the runtime continuation and then returns to the scheduler.

Sources: [ETS exit continuation](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14360-L14366); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14360); [budgeted relationship and signal cleanup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14534-L14596); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14534); [save exit continuation and yield](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14795-L14832); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14795); [continue-exit scheduling bridge](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/jit/x86/instr_common.cpp#L287-L300); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/jit/x86/instr_common.cpp:287).

Exit processing must still fulfill communication obligations. For an established local target-monitor endpoint, the exiting process sends DOWN with its reason; for a monitor it owns as observer, it sends demonitor to the watched process. Active local links send linked exit signals. Unprocessed late monitor and link requests receive `noproc`; an unlink operation reaching the exit-drain path still receives its acknowledgment.

Sources: [notify observers](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13798-L13815); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:13798); [detach observations owned by the exiting process](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13891-L13898); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:13891); [notify links](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14068-L14083); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14068); [late relationship obligations during exit](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_proc_sig_queue.c#L7347-L7385); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_proc_sig_queue.c:7347).

**Limit:** this is not evidence that every cleanup instruction has a bounded cost. The final `delete_process` function cleans pending message queues, off-heap references, heaps, and dictionary storage without an exit-continuation budget check in that function. Its callees and native resource destructors require their own latency and safety audit. There is no measured hard bound here.

Source: [final storage cleanup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L13393-L13482); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:13393).

**Bream implication:** killing a process with many messages, timers, or relationships must not simply move the original CPU starvation into a huge destructor pass. Define incremental cleanup units and test fairness during reclamation as well as during application execution. Runtime-owned cleanup should call a restricted, audited set of operations, not arbitrary user `Drop` implementations.

## 7. DOWN means a lifecycle milestone, not universal reclamation

The source gives a useful ordering:

1. Release directly visible resources; the registered name is intended to be the final such resource.
2. Detach relationship roots, remove the record from the process table at the source's “Time of death,” and mark it FREE.
3. Traverse relationships and send exit/DOWN signals.
4. Finish remaining exit work and call `delete_process`, or defer that deletion while dirty native execution still holds the process.
5. Eventually free the record after its reference count reaches zero.

Sources: [visible-resource release and table removal](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14420-L14469); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14420); [subsequent notifications](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14545-L14584); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14545); [deferred deletion and final cleanup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L14755-L14791); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:14755); [final record release](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L12142-L12149); [local](/home/bat/projects/bream/upstream/otp/erts/emulator/beam/erl_process.c:12142).

The public reference explicitly excludes heap-held resources, dirty native execution, and the process identifier from the directly-visible-resource promise. Dirty native code may still be executing after the process is considered exiting; ERTS cannot forcibly stop arbitrary native code. Heap storage generally must remain until the process has sent its required signals, including terms used in exit reasons. The API contract separately says PID reuse must wait until everything regarding the process has been released.

Source: [notification and resource-release contract](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/system/doc/reference_manual/ref_man_processes.md#L458-L493); [local](/home/bat/projects/bream/upstream/otp/system/doc/reference_manual/ref_man_processes.md:458).

This source ordering does **not** imply that every observer receives DOWN before storage is freed: observers run concurrently and may process it much later. It establishes that the runtime can send the notification before full reclamation. Nor does a `noproc` response from a monitor installed too late certify that this observer witnessed the target's original exit reason.

**Bream implication:** the user has now selected DOWN after execution has stopped and identity has been removed, with tracked cleanup completing separately. This is intentionally stronger than BEAM's dirty-native exception: Bream must not emit DOWN while an admitted application helper continues executing arbitrary native work. Distinguish the request to kill, irreversible stop, DOWN, and final tracked cleanup completion in implementation and tests. The exact completion API and which visible obligations precede DOWN remain to be specified. Keeping runtime-owned buffers alive until outstanding runtime I/O releases its references is essential; indefinitely quarantining arbitrary native execution is not the accepted substitute for safe forced termination. The selected immutable-byte sharing also means process death releases that process's references, rather than claiming every shared allocation has become globally reclaimable.

## 8. Falsifiable Bream probes

These are proposed tests, not results and not an implementation plan already approved by the user.

| Probe | Failure that would disprove the intended contract |
|---|---|
| Reuse one table slot repeatedly; retain old handles and model generation exhaustion with a deliberately tiny counter. | An old incarnation's send, monitor, or kill affects a replacement before the documented reuse boundary; record storage is freed while a lookup still owns a reference. |
| Spawn with an initial monitor/link; make the child's first action exit. Compare with deliberately separate spawn then monitor. | Atomic spawn loses or duplicates the relationship outcome; its implementation exposes the same registration gap as the non-atomic sequence. |
| Race target death with demonitor before signal handling, after notification materialization, and after notification consumption. | A cancelled relation creates a later notification; flush removes an unrelated message/reference; either endpoint leaks or is released twice. |
| Cross unlink, relink, target death, and a delayed old unlink acknowledgment. | An old acknowledgment removes the new link, an inactive link propagates death, or unlink silently removes an already-materialized trapped event contrary to the chosen contract. |
| Exercise the complete exit table with a CPU-loop target, suspended target, and trapped target; distinguish direct kill from a linked reason named kill. | Forced kill is catchable or waits for application receive; the user continuation resumes after the irreversible exit transition; propagated reasons are misclassified. |
| Kill processes holding large mailboxes and many monitors, links, timers, and runtime resources while an unrelated process remains runnable. | Cleanup monopolizes a worker without the documented bound, loses protocol replies, or completes with nonzero tracked owned resources. Measure costs by phase rather than infer them from API return time. |
| Observe death while a supported asynchronous runtime operation owns a buffer. | A death notification is mistaken for permission to free a buffer still in use; the final cleanup completion fires too soon or never arrives. Verify visible-resource availability separately from storage reclamation. |
| Arrange new link/monitor/unlink requests at every exit-cleanup phase. | The runtime drops ownership-bearing control messages without finishing or cancelling their protocol, delivers to a replacement incarnation, or leaves a peer waiting forever. |

Testing should initially use a deterministic interleaving model for relationships and ownership, plus native stress tests. Passing stress tests alone does not prove the absence of a race. The generated-process probe also needs closure of its supported call/type/operator surface; lifecycle correctness cannot repair a helper that never reaches a runtime checkpoint.

## Questions for the next grilling round

These were suggestions from the source review, not questions asked to the user. The later Q21 instruction resolves ordinary semantic preferences in favor of BEAM: preserve queued DOWN unless flushing is explicit, and preserve an exposed exit operation's actual self-exit behavior. Identity representation still needs a Rust-specific safety analysis. These notes do not reopen safe kill or the accepted separation between DOWN and tracked cleanup completion.

1. **Cancelling observation:** should the convenient demonitor operation also flush a queued notification by default, or preserve it unless flushing is explicit? In either case, retain a precise reference-scoped operation and distinguish cancellation from consuming history.
2. **Identity exhaustion:** must a stale handle remain invalid for the entire runtime instance, accepting explicit allocation failure/slot retirement at generation exhaustion, or may the contract permit eventual full-ID reuse as Erlang does? This is a semantic choice even if a wide counter makes exhaustion remote.
3. **Self exit:** should Bream distinguish terminating oneself from sending an exit signal to a PID, avoiding the legacy `exit(self(), normal)` special case, or intentionally preserve that Erlang API behavior? The accepted linked-exit/trapping matrix can remain unchanged under either interface.

## Coverage and remaining gaps

This report followed local creation and relationship installation, full-ID lookup checks, ordinary process-monitor endpoints, local link/unlink signaling, direct/linked exit classification, the exit continuation, and the ordering of visible death versus storage cleanup. It checked the x86 JIT bridge for continuation scheduling.

It did not establish linearizability of every interleaving or every memory ordering; audit distribution/connection IDs, priority reception, aliases, monitor tags, ports, resource monitors, or suspend monitors; inspect every allocator and reference-count primitive; prove bounds for final queue/off-heap destruction; or validate dirty scheduler and NIF resource safety. Native operations and memory subsystems need the companion reviews. No production Bream soundness, unrestricted Rust cancellation, hard latency guarantee, or full BEAM compatibility follows from this source walkthrough.
