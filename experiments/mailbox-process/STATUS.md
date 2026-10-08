# Owned mailboxes and parking

Passed for the restricted single-worker, fixed-tag/any-selector profile, 2026-10-08. [Implementation and limits](README.md), [retained correctness cohort](../../evidence/mailbox-process/runs/20261008-mailbox-01/results.json).

All 155 expected command outcomes passed: 21 unit tests per compiler profile, six public control invocations, 132 expected compile rejections and six shadowing executions, plus builds. Controls establish owned transfer, preserved nonmatches, resumable scan work, parking/wakeup, waiting-state kill, independent allocation receipts and batched queued-message cleanup. No local speed test, benchmark or performance comparison ran.

Earlier source/evidence remain unchanged. G1/G2/G3 remain partial; receive timeout correctness with an injected clock is next. The no-speed-test machine rule is persistent in the host and project agent instructions.
