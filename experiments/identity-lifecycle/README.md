# Stable process identity and terminal lifecycle

**Passed for the declared single-worker profile, 2026-10-08.** This extends the [structured-state runtime](../structured-process/README.md) in a separate workspace. The checked compiler is unchanged except for its runtime crate binding. Implementation inputs and correctness evidence remain; performance evidence was removed at user request. [Milestone tracker](../../docs/milestones.md), [retained evidence](../../evidence/identity-lifecycle/README.md).

## Identity, publication and ownership

The separate [consumer](consumer/src/main.rs) uses copyable `ProcessId` values containing a private runtime namespace, slot and generation. A mutex-protected registry checks all three before returning a retained diagnostic handle. `is_live(id)` borrows under that guard and returns a momentary observation, never a lease or a safe precondition for a later operation.

[Lifecycle implementation](runtime/src/lifecycle.rs):

| Transition | Experimental contract |
|---|---|
| Prepare | Build the complete generated native frame before allocating its identity. Host allocation precedes publication. |
| Publish | One registry critical section installs the record and transfers the runnable task into the worker channel. No lookup can observe a failed enqueue. |
| Reject | Capacity, closed admission or disconnected enqueue destroys the prepared frame and returns its cleanup snapshot. Oversized input is rejected before tracked frame construction. Input is consumed on error in this experiment. |
| Lookup | Match runtime, slot, generation and occupancy; retain metadata while protected. The returned handle does not own the application frame. |
| Kill | Return `Accepted`, `AlreadyRequested` or `TooLate`. A lifecycle lock arbitrates requests against terminal commitment; accepted pending kill wins over uncommitted completion/failure. |
| Stop | Under registry → lifecycle lock order, remove the exact record, return its slot with an advanced generation, then publish execution stop and its immutable reason. No further application polling. |
| Cleanup | Destroy the old frame outside both locks, then publish cleanup completion on its old metadata. New slot occupants are untouched. |
| Shutdown | Seal admission and enqueue shutdown under the publication lock. Join after accepted queued/running work is stopped and destroyed. |

A retained handle keeps diagnostics alive without keeping application execution alive. Dropping the last external handle does not kill a process; the runtime still owns it. A copied ID retains no record at all. A stale ID cannot acquire the replacement's record. A stopped handle remains inspectable and repeated kill cannot change its snapshot.

The namespace counter uses checked increment; exhausting it fails runtime creation. An exhausted slot generation permanently retires that slot. These are explicit stricter prototype policies, not BEAM's finite-wrap representation. Capacity defaults to 1,024 and can be set to 1–65,536 for the experiment. The free-slot stack avoids scanning occupied entries; no global registry lock is acquired for ordinary process polling.

The prior worker's 32-command intake limit and per-poll checkpoint allowance remain. Host publishers can still contend on registry locks. The bound on intake count is not a wall-clock bound.

## BEAM source obligations

The source audit revisited pinned OTP `ad05823719d77c8faee87348ea39513d4e2f99c5` directly; it did not run a new VM fixture.

- Full PID validation and rejection of exiting records: [erl_process_lock.h](../../upstream/otp/erts/emulator/beam/erl_process_lock.h:1135).
- Clear the old table pointer and advance the identity before returning a free token: [erl_ptab.c](../../upstream/otp/erts/emulator/beam/erl_ptab.c:694).
- Remove the live identity before death notifications, with record release separately scheduled: [erl_process.c](../../upstream/otp/erts/emulator/beam/erl_process.c:14455) and [later release](../../upstream/otp/erts/emulator/beam/erl_process.c:14765).
- Initially locked child construction and recoverable body/table allocation failures: [erl_process.c](../../upstream/otp/erts/emulator/beam/erl_process.c:12159). This does not establish recovery from arbitrary later allocation failure.

The Rust implementation publishes fully prepared frames, uses native ownership to retain exact old records and separates stop from destruction. Kill receipts, nonwrapping generations, rejected-input consumption and host shutdown errors are reversible experimental API choices. They do not change the accepted BEAM observable-semantics default. [Existing source analysis](../../docs/research/39-process-table-and-spawn-publication.md) and [lookup/reclamation analysis](../../docs/research/27-process-lookup-reclamation-protocol.md) explain the broader obligations.

## Controls

The [11 internal tests](runtime/src/lifecycle/tests.rs) force the protocol orders that scheduling luck cannot guarantee: kill before terminal claim, claim before kill, actual Ready poll followed by accepted kill, stop followed by slot reuse before old cleanup, shutdown between preparation/publication, accepted enqueue before shutdown, disconnected enqueue rollback, capacity rejection, full-ID checks, generation exhaustion and namespace exhaustion. The injected failure-reason case exercises arbitration; it is not an actual panic racing kill.

Separate consumer controls run in debug and release at allowances 1, 32 and 256. Each invocation includes:

- Queued and running kill; foreign-runtime rejection; live replacement isolation; dropping the last handle while retaining only an ID.
- 128 sequential reuse cycles with retained historical handles, correct owning-record transforms, a real bounds panic and a surviving peer.
- 200 host kill/completion races alternating small and larger transforms. The returned receipt determines the required outcome; both winners need not occur in every run.
- Eight concurrent publishers competing for four slots: four accepted identities and four rejected frames, with no duplicate ownership.
- Four publishers racing shutdown, plus an accepted anchor and a post-shutdown rejection; all 33 racing/anchor attempts settle exactly once.
- Explicit shutdown and Runtime drop with queued/running owning frames; closed clients remain safe afterward.

The earlier structured and buffer controls also run against the new runtime. Consumer compilation rejects 57 fixtures per profile: the 55 inherited admission fixtures plus attempted identity construction and mutation. Both lexical-shadowing routes execute in both profiles.

Run individual controls using the retained binaries:

```sh
.tools/identity-lifecycle/20261008-identity-01/debug/bream-identity-consumer controls 32
```

The controls intentionally print caught panic diagnostics. JSON must still report `passed`. Internal protocol fixtures are compiled only under `cfg(test)`; the public workload and measurement binaries expose no pause-between-stop-and-cleanup hook.

## Correctness retained; performance requires a new run

The [filtered cohort](../../evidence/identity-lifecycle/runs/20261008-identity-01/results.json) retains 145 commands: eleven unit tests in each profile, eighteen consumer controls, 114 expected compile rejections and four shadowing executions, plus builds. Identity controls cover 1,200 kill/completion races with receipts checked against terminal outcomes.

**Performance evidence removed, 2026-10-08, at the user’s request. All speed tests must be redone in an authorized suitable environment. No speed tests may run on this machine.**

Registry, process-churn and borrowed-versus-retained lookup measurements, ratios and inferred wins/regressions have been deleted. Source still shows that scoped liveness observation avoids retaining a diagnostic handle, but its speed and contention behavior need fresh qualification.

## Remaining limits

Shutdown waits for accepted worker work. A concurrent host can still finish preparing an unadmitted frame afterward, receive `Closed` and destroy that rejected frame. Unexpected worker failure is unrecovered: accepted metadata may remain unsettled and shutdown panics. Allocation failure, allocator behavior and host panic hooks remain outside the recovery claim.

Stopped observation follows live-table removal and precedes completion of frame cleanup. Natural completion and panic can already have dropped some values inside a poll. No monitor `Down`, waiting-state cancellation, mailbox ownership, multiworker reclamation or full G1/G2 qualification is implemented here. Paused runtimes are a control facility: queued kill settles only after start or shutdown.
