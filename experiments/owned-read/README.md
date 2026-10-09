# Runtime-owned local read

**Fourteenth bounded milestone passed for the declared local single-worker profile, 2026-10-09.** This experiment extends [explicit exit](../explicit-exit/README.md) with one bounded external read owned by the runtime independently of actor execution. It applies the [historical Rust investigation](../../docs/research/63-rust-green-runtime-lessons-and-experiments.md) and [pinned source contract](../../docs/research/66-owned-read-source-contract.md). Broader G1–G5 qualification remains open.

**Correctness only.** No speed tests, throughput/latency measurements or performance comparisons run here. Watchdog deadlines only detect hangs; configured allowances vary checkpoint boundaries. [Standing machine constraint](../../AGENTS.md).

## Admitted operation

The trusted host creates a runtime with `Runtime::with_read_capacity(allowance, process_capacity, operation_capacity)` and consumes one local `UnixStream` through `client.install_read(pid, endpoint, size)`. Installation requires a live exact process incarnation with no fatal exit already accepted. The fixture gates the application before it asks for its input; this is not an atomic spawn-with-resource API.

One endpoint can be installed per actor. The operation begins on installation, independently of when the actor calls checked `read_input()`. Its result is one of:

| Result | Meaning |
|---|---|
| `ReadResult::Data(ReadBytes)` | One positive read completed. The owned buffer contains exactly the returned bytes; a short read is a complete result. |
| `ReadResult::Eof` | The peer ended the stream before a positive read. Admission forbids zero-size requests, so zero means EOF. |
| `ReadResult::Failed` | The nonblocking read returned an error other than WouldBlock or Interrupted. Platform errno details are deliberately omitted. |
| `ReadResult::Unavailable` | No installed input, a second consumption attempt, or loss/cancellation of the adapter before delivery. |

`read_len(bytes)` and `read_byte(bytes, index)` expose immutable access. ReadBytes can move into checked records and helpers; they cannot be cloned or sent as an existing shared-byte envelope. The macro generates the wait without admitting arbitrary futures or user-defined I/O callbacks.

This is a **one-shot endpoint**: the first positive read, EOF or failure closes its local endpoint. A short read does not preserve the endpoint for unread trailing data. There is no read-exactly operation, repeated-read handle, networking framework or timeout API here.

Admission allows 1–16 pending or ready-but-unconsumed operations and 1–4096 bytes per request. A ready result keeps its operation slot until consumed or discarded. Bytes already moved to an actor follow that actor's ownership and no longer require an operation slot. These limits are not a whole-runtime memory cap. Rejected installation consumes/closes the supplied endpoint.

The host must preserve exclusive access to the admitted side: an earlier `try_clone` can still share stream state and nonblocking options. Consuming a Rust handle does not prove that no host alias exists. Keeping the opposite socketpair endpoint to supply input is supported.

## Ownership and cancellation

The [driver](runtime/src/io_read.rs) owns a private operation record, socket and initialized buffer. It uses one dedicated thread and a private wake socketpair. A service pass performs at most one nonblocking read per admitted operation. WouldBlock or Interrupted retains the request for a later pass; there is no loop filling a buffer inside one attempt.

Before reading, the driver moves the whole request out of its shared state and releases the state lock. Neither the syscall nor the readiness wait holds an actor-frame borrow. Only this driver closes admitted sockets, so descriptors remain owned throughout a poll snapshot. After a successful read, the same Vec allocation moves into ReadBytes. A unit control checks allocation identity across that transfer; it makes no speed claim and does not remove the kernel-to-buffer copy.

Actor stop marks cancellation and proceeds with existing frame, mailbox and relationship cleanup. It never joins the driver or frees a request the driver has claimed. Cancellation observed after claiming can suppress the read. Once the driver has passed that check, the read may consume bytes after actor death, then discard them. **Cancellation does not roll back external I/O.**

Completion belongs to the original operation object and original full namespace/slot/generation. It cannot look up a replacement by slot alone. A pending request can therefore outlive actor cleanup without retaining the actor's frame. The deterministic late-completion test stops the actor, reuses its slot, keeps the request buffer alive, then completes/discards that old request without changing the replacement. Duplicate completion cannot replace the first result.

Result publication and wait enrollment cooperate with the mailbox lock. Publication releases the operation lock before waking the actor. Host shutdown seals actor admission, stops the scheduler, closes the read hub and joins the driver from the host. Pending reads do not wait for remote peers to close.

## Driver failure and review fixes

A wake sends one byte through a nonblocking control socket. WouldBlock means a wake is already pending. Any other write error, including Interrupted, drops the sole writer. Persistent EOF/HUP wakes the driver, which fails closed and settles outstanding operations. This deliberately sacrifices the adapter on control failure instead of retrying indefinitely inside actor lifecycle locks. A fault-injection control closes the writer and checks settlement, wakeup and rejected subsequent admission.

Review also found cancellation was initially sampled before claiming a request. The driver now samples after claim, so cancellation completed before claim cannot be bypassed by an older sampled value. A controlled ordering test covers cancellation between claim and the final check.

The implementation assumes ordinary trusted-host allocation, locks, syscall and panic behavior. Nonblocking mode prevents waiting for peer input; it does not prove bounded kernel execution, allocator work or OS scheduling. Worker/driver panic recovery and multiple application workers remain unqualified.

## Validation

The [public consumer](consumer/src/reads.rs) checks pending and precompleted data, EOF, missing input, independent peer progress, capacity retained by an unconsumed result, same-slot replacement and shutdown with peers open. It separately stops actors while waiting for data, while a completed result awaits consumption, and while a helper owns returned bytes. Nineteen unrelated queued messages retire in three cleanup batches in each stop case. Six configurations cover debug/release at allowances 1, 4 and 32; each includes sixteen read/kill races.

Fourteen [new runtime tests](runtime/src/io_read/tests.rs) add controlled claimed-request lifetime, duplicate completion, admission errors, buffer transfer identity, abandoned waits, mixed pending/ready shutdown and control-channel failure. The failed-result and duplicate-completion controls inject internal outcomes; they are not observations of a real kernel read failure or duplicate kernel callback. Real socket observations cover data, a positive short read, buffered data with peer closure, EOF, WouldBlock and shutdown.

The [passing cohort](../../evidence/owned-read/runs/20261009-read-01/results.json) contains 454 expected command outcomes, 408 aggregate observations and 219 frozen inputs. Both compiler profiles pass 119 runtime tests, 169 rejected consumer programs and ten positive shadowing examples. All inherited public controls and the prior BEAM exit reference also pass. The [read-only audit](../../evidence/owned-read/validation.json) verifies 1,157 hash records, all current inputs and all 205 preceding-milestone inputs. The [evidence guide](../../evidence/owned-read/README.md) records the scope. It does not claim a matching Erlang socket API or general asynchronous-I/O cancellation.

Next qualification should make this resource useful across repeated reads while retaining exclusive endpoint ownership and exact operation identity. That requires a persistent endpoint lifecycle, bounded per-endpoint admission and stale readiness/completion tests beyond this one-shot profile.
