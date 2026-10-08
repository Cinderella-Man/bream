# Native preemptible functions: useful prior art

Reviewed 2026-10-07. This is source research, not a successful build or performance replication.

## What the research establishes

Boucher, Kalia, Andersen, and Kaminsky's ATC 2020 paper presents **libinger**, a Rust/C interface that runs a function for a time allowance, returns its continuation when interrupted, and later resumes it. It also presents **libturquoise**, built by adapting a Rust futures thread pool. This directly contradicts the categorical claim that a native library cannot implement resumable preemption.

The mechanism uses timer signals and separate native stacks. The harder contribution is handling nonreentrant libraries: selective relinking copies certain library state, while operations that cannot be treated that way defer interruption. The paper's implementation used fixed 2 MiB stacks and did not automatically release all resources acquired by cancelled functions. These are concrete compatibility and density costs, not reasons to confuse interruption with cooperative application code. [Primary paper, especially §§3–5](https://www.cs.cmu.edu/~sboucher/atc20.pdf), [USENIX publication](https://www.usenix.org/conference/atc20/presentation/boucher).

## Why not adopt it as-is?

The current repository README restricts support to x86-64 GNU/Linux and lists old toolchain/system dependencies, notably glibc ≤2.33 and an older `libebl`. It explicitly warns that the glibc 2.34 library merge likely prevents an unchanged build. This host reports glibc 2.43. I did not attempt to replace system libraries or port its linker machinery during this investigation. The README also documents that tasks execute on their caller's kernel thread, with corresponding lock/deadlock concerns. [libinger source and build contract](https://github.com/efficient/libinger).

The research group's index links both the original preemptive Tokio fork and later thesis implementations. “There is a paper with a Rust API” should therefore not be treated as “there is a current drop-in Cargo dependency.” [Research artifact index](https://efficient.github.io/#lpf), [original libturquoise/Tokio fork](https://github.com/efficient/tokio).

## Implications for Bream

These are design inferences, not new experimental results:

- A useful low-level abstraction is **run a native continuation for a turn, regain control, resume later**. The higher layer can implement process queues, actor mailboxes, or CPU jobs.
- Expose resumable scheduling independently from cancellation. The core use case does not require discarding arbitrary Rust stacks.
- Library/global/TLS state deserves equal attention to saved registers. Merely moving the scheduler to a different stack does not erase the interrupted library's invariants.
- Selective relinking is an alternative worth understanding, but it creates a substantial system integration burden. It is not the recommended first implementation for a small portable Cargo library.
- Benchmark reserved stack space, touched RAM, supporting state and OS threads separately before using “tiny” as a quantified claim.

The [native mechanisms report](02-native-mechanisms.md) covers another relevant alternative: preserving the interrupted kernel thread and handing execution capacity to another thread. The [landscape](03-runtime-landscape.md) covers currently packaged native Rust libraries, and the [local probe](../../experiments/native-preemption/README.md) supplies fresh mechanism evidence.
