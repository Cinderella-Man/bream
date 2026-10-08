# Useful process milestone

This is a finite experiment toward G1/G2, authorized on 2026-10-08. It extends the earlier mechanism into a separately compiled consumer with useful owned-data work. Historical probes and their evidence stay unchanged.

The milestone passes only when all of these have retained evidence:

- A checked program processes an owned input buffer and produces the independently expected result, without authored yields or cancellation checks.
- A nonterminating data-processing process repeatedly shares one application worker with a finite peer.
- An external host request stops the nonterminating process; no later poll or progress occurs after committed stop.
- Stop and completed cleanup are separate observations, with exactly-once release of tracked input storage.
- Ordinary safe consumer code cannot pass an arbitrary future/closure or forge a program through the supported public API; unsupported source fixtures fail compilation.

**Passed for this restricted profile.** [Run 02](../../evidence/process-milestone/runs/20261008-useful-process-02/results.json) retains the separate-consumer checks, correctness controls. Independent review found and corrected a backing-capacity admission gap; run 01 is preserved. [Results and limits](README.md) identify exactly what passed. Full G1/G2 remain partial.

This does not close all of G1/G2. User-defined structures/enums and helper calls, messaging, waiting I/O, a reusable identity registry, general allocation/failure isolation and multiple workers remain outside this slice. The input is a concrete owning array of primitive words with a fixed experimental size limit. Host allocation and deallocation have no hard wall-clock bound.

**Performance evidence removed, 2026-10-08, at the user’s request. All speed tests must be redone in an authorized suitable environment. No speed tests may run on this machine.**
