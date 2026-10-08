# Predeclared PERF-091 observation protocol

Original plan from 2026-10-08; all associated results were removed. This is a redesign input for a new authorized environment, not permission to run locally. This narrows CL51-T01 to the cost of
ordinary view cloning, retention, reading and release. It does not qualify
Bream or fully execute PERF-091.

## Contract and controls

Every timed round clones `fanout` views from two immutable sources, retains all
views until all their checksums have been consumed, and then releases them.
Source owners survive all rounds. Representations must return identical bytes
and aggregate checksums. Full-byte equality, surviving-owner/drop-order checks,
and final `Arc` weak-upgrade failure run outside timing in each fresh process.
The program forbids unsafe code. No application kill or destructor suppression
is modeled. Weak-upgrade failure establishes the end of strong ownership; it
does not measure allocator reclamation time, and a weak owner itself remains.

Inputs are fixed named static arrays of 8/256/4096 bytes and fanouts 1/16/256.
Two deterministic byte patterns alternate between sources. Workloads are all
static, 50% static plus 50% dynamically allocated, and all dynamic. The mixed
case is a deterministic alternating pattern, not a random branch distribution.
The general baseline uses `Arc<[u8]>` for every source. The treatment's private
`Static`/`Shared` enum uses an `Arc` for dynamic data; direct static slices are
an additional all-static control and are never used for dynamic data.

Construction/allocation of the source buffers and retained-vector capacity,
final source destruction, and full payload traversal are excluded for all
arms. The same fixed digest reads first/middle/last bytes plus the length of
each view. Payload size is a handle/layout input here, not a throughput sweep
for copying or reading every byte. The vector is reused and preallocated;
its cloning, push, iteration and clear work remains timed. `black_box` hides
inputs and exposes retained views and output. No empty-loop subtraction.

## Sampling and build

Nine independent processes each execute all 63 configurations, warming each
configuration for at least 100,000 views before timing at least 2,000,000 views.
Fanout rounding is retained in raw output. Odd samples reverse workload order;
the two-arm order alternates, and three-arm order rotates/reverses. Correctness
assertions check every measured aggregate checksum. No timed pilot selects
the final input count. Samples are fixed in advance: no optional stopping,
outlier rejection or selective reruns.

Rust 1.98.1; edition 2021; `-C opt-level=3 -C codegen-units=1 -C lto=fat
-C panic=abort -C target-cpu=x86-64 -C debuginfo=0`. The runner captures compiler,
input and binary hashes, timestamps, arguments, working directory, inherited
environment-key names and selected non-secret build/runtime overrides. No
native CPU code generation is requested. Each process is pinned to CPU4 when
available, otherwise the first allowed CPU. CPU topology, governor, affinity,
load and host metadata are retained. Pinning does not provide exclusive core,
SMT sibling, fixed frequency, temperature or machine isolation. Other research
may run on distinct physical cores and other uncontrolled jobs may coexist.
The root notification probe is planned for CPUs0/1; CPU4's SMT sibling is20.

Performance counters are probed once, and failures are retained. If unavailable,
no cycles/instructions/cache/branch/atomic-count result is claimed. Program-wide
binary section sizes and representation sizes are recorded, but are not a
comparative code-size estimate; there is one binary containing every arm.

## Acceptance and noise rule

Retain every raw stdout/stderr and exit status. Any failed correctness check,
missing configuration, differing matched checksums, fewer than nine samples,
or changed input/compiler/binary identity invalidates the run. Publish every
sample plus median, linearly interpolated p25/p75, minimum and maximum in
nanoseconds per view. Pair treatment/baseline by process and identical input.

A directionally consistent *local observation* requires both arms' median
elapsed time to be at least 5 ms, median paired improvement greater than 10%,
and at least eight of nine pairs improving by more than 5%. A regression uses
the symmetric thresholds. Everything else is inconclusive under this rule.
No statistical significance or population confidence interval is claimed;
variation outside this host/workload remains unknown. Use relative improvement
`100 * (1 - treatment_ns / baseline_ns)`, not the ratio of pooled totals.

Even a passed local observation leaves allocation counts, construction costs,
full traversal, cache contention, cross-thread atomics, scheduling, message
delivery, resource accounting, kill-to-stop and cleanup unmeasured. The private
enum is an experiment, not a selected production representation or proof of
hostile-code admission.
