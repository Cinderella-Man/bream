# Linux I/O lifetime observations

These are isolated Linux syscall probes supporting [report45](../../docs/research/45-linux-io-observations-and-rust-boundary.md), alongside source reviews [42](../../docs/research/42-linux-readiness-and-registration-lifetime.md), [43](../../docs/research/43-process-enumeration-and-observer-lifetime.md) and [44](../../docs/research/44-native-resource-callback-and-owner-lifetime.md).

Run from the project root with `python3 evidence/io-lifetime-review/run_probes.py`. The runner compiles [epoll_contract.c](epoll_contract.c) with warnings treated as errors at `-O0` and `-O2`, then launches each of five scenarios in a separate child with a five-second watchdog. Binaries live in ignored `.tools/io-lifetime-review/`; [probe-results.json](probe-results.json) records their hashes, compiler identity, host kernel, commands, expected observations and raw output paths. [Raw outputs](runs/) are retained.

Recorded result: **2 compilations and 10 executions, all expected observations**. Families cover copied event identity through descriptor reuse, duplicate-descriptor registration lifetime, consumed readiness returning EAGAIN, one-shot rearming and hangup with unread data.

These sequential probes do not implement Bream, invoke BEAM, test concurrent memory ordering, establish a complete I/O cancellation protocol or measure performance. A copied event remaining in a local variable is an intentionally simple negative control for cancellation-as-retroactive-erasure. Real concurrent dispatch, teardown and generation-wrap qualification remain planned.

[Prior case IDs](prior-case-ids.json) and [prior task IDs](prior-task-ids.json) preserve the 447-case/54-task catalogue before this pass. [Prior evidence hashes](prior-evidence-hashes.json) retain historical records. The new [validation record](review-validation.json) checks this pass without rewriting previous results.
