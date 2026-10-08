# Process exceptions and executable-code lifetime

Deeper source review, 2026-10-07. Pinned OTP `29.1.1`, commit `ad05823719d77c8faee87348ea39513d4e2f99c5`. These are process-runtime boundaries rather than bundled utility algorithms. Exception behavior affects the initial Rust mapping; hot code replacement remains later research, not an initial Bream requirement. No reference or Bream tests were executed.

## An exception is not automatically process death

The common runtime path `handle_error → next_catch or terminate_proc → erts_do_exit_process` separates language exceptions from irreversible lifecycle exit. Interpreter and JIT-generated exception paths reach common runtime machinery. The key branches are visible in [handle_error](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_common.c#L449-L579):

- Restore native-function error context when a scheduled native operation raised.
- Separate the actual reason from optional argument/error-info metadata.
- Save the trace before a rethrow can change the final exception classification.
- Convert an uncaught `throw` into a `nocatch` error.
- Expand and stabilize the reason, clear transient process-held error data, then locate a handler unless the runtime's non-catchable panic flag forbids it.
- Resume the selected catch/try continuation when one exists; otherwise terminate.

`next_catch` walks continuation and tracing frames as well as catch markers. It updates the surviving stack boundary, clears the consumed marker and emits relevant tracing events. This is a stack/metadata ownership transition, not a call to arbitrary application cleanup code. [Catch search](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_common.c#L584-L678).

When no handler exists, `terminate_proc` attaches a symbolic trace to an error-class reason, optionally reports it, clears trace/register liveness and enters lifecycle exit. An explicit uncaught exit reason and an error with a trace therefore need not have the same envelope shape. The `EXF_PANIC` name in C is an ERTS internal exception flag; it is not the Rust panic mechanism. [Termination handoff](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_common.c#L685-L735).

For Bream, a typed operation error, a catchable process exception, an unhandled process failure, direct forced kill, and a Rust panic require separate treatment. The accepted native process model need not recreate every Erlang syntax form, but an API claiming `exit/1`-like behavior cannot silently use uncatchable kill. Likewise, `catch_unwind` neither supplies Erlang's three exception classes nor guarantees safe handling of panic hooks, abort builds or arbitrary destructors. Existing LIFE-04 and EXEC-04 cases remain applicable; LANG-04 below makes the mapping work explicit.

## Code lifetime can retain or terminate processes

BEAM's `check_process_code` is process inspection over executable references. In the pinned implementation it checks a module's old code under its code lock, then looks at the current instruction, native-function wrapper state and continuation pointers on the stack. Missing module or missing old code returns false. References retained in exception trace storage are cleared when they point into the affected code. [Module/lock handling](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_bif_load.c#L593-L631), [direct references and trace cleanup](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_bif_load.c#L1149-L1218).

The public documentation contains historical terminology: its change note explicitly says modern checks inspect direct code references, while old fun references may instead fail when invoked after purge and literals are dealt with separately. Do not use the older broad wording as evidence that the modern function scans every heap value. Async requests return a correlated result message; synchronous requests for another process wait for its runtime work. [Contract and version qualification](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erlang.erl#L1608-L1689).

Dirty-process checking has a restricted caller and distinct absent, normal-state and busy-lock branches. It must not read a mutating native continuation merely because a PID exists. The scheduler also services code checks as system tasks and has separate handling for work delayed by GC state or delegated to dirty execution. [Dirty check](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/beam_bif_load.c#L634-L679), [system task dispatch](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/emulator/beam/erl_process.c#L10859-L10905).

The preloaded code purger consumes these primitives: soft purge aborts on a process still referring to the old code; hard purge schedules kills, monitors completion and limits outstanding kills. It explicitly uses a bounded number of simultaneous kills to avoid flooding its own mailbox and retaining too many monitor references. This is a useful raw-process consumer even though Bream is not adopting Erlang's code server. [Purger state machine](https://github.com/erlang/otp/blob/ad05823719d77c8faee87348ea39513d4e2f99c5/erts/preloaded/src/erts_code_purger.erl#L186-L282).

These paths explain why code replacement is process-related, but they do not make it a mandatory port. A statically linked Rust runtime can exclude dynamic code replacement. If replacement is later admitted, native continuation/function-pointer lifetime, outstanding helper execution, stored callbacks, diagnostics and data migration each need an explicit design. Swapping a function pointer is not a hot-upgrade protocol.

## Additional branch cases

| Case | Branch and required qualification |
|---|---|
| LANG-04.C01 | A supported operation raises an error that is caught; continue with valid owned state and correct exception class/reason, without turning it into runtime kill. |
| LANG-04.C02 | Explicit exit is caught or uncaught; direct kill arrives during catching; preserve the separate lifecycle behavior and the untrappable forced-stop guarantee. |
| LANG-04.C03 | An uncaught throw/error or supported rethrow ends execution; map reason and trace metadata without leaking retained payloads or claiming ordinary Rust panic equivalence. |
| LANG-04.C04 | Exception handling/tracing crosses suspended helper state or fails during diagnostics; release each ownership once and retain the declared host-fatal boundary. |
| CODE-01.C01 | Check code with missing module/no old version, current instruction, saved return address or native wrapper reference; distinguish direct executable references from ordinary data references. |
| CODE-01.C02 | Code inspection races execution, dirty work, death or delayed system tasks; no reading or reclaiming a still-used continuation without the proper ownership barrier. |
| CODE-01.C03 | Soft/hard replacement sees a live old-code user; refuse or terminate according to an explicit contract, and observe kill completion before unsafe reclamation. |
| CODE-01.C04 | Old closures, literal storage, exception traces or diagnostic locations outlive code replacement; reject unsupported native upgrades or specify lifetime and failure behavior before exposing them. |

LANG-04 refines an initial mapping obligation; CODE-01 is later research. Coverage here is the exception handoff and the process/code boundary, not all exception opcodes, every JIT unwind path, literal-area copying, module loading or the complete purger implementation.

Plan integration: these case IDs are recorded in the [expanded build catalogue](../build-plan/process-runtime-expansion.md). All remain planned, with conditional applicability preserved.
