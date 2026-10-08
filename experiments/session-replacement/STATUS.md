# Session replacement and stale incarnation isolation

Passed for one bounded replacement in the fixed two-member single-worker profile, 2026-10-08. [Implementation and limits](README.md), [cohort](../../evidence/session-replacement/runs/20261008-replacement-01/results.json), [audit](../../evidence/session-replacement/validation.json).

All 387 expected command outcomes passed, with 352 observations and 187 retained inputs. Six replacement configurations cover twenty-four continuing/teardown scenarios. Each compiler profile also retains 95 runtime unit tests, 148 expected compiler rejections and eight positive shadowing examples.

A three-slot registry forces a replacement to occupy the old member's vacated slot. The room requires a fresh full PID and monitor reference. Old reply and genuine Down remain queued while replacement readiness is selected; a stale trusted-epoch request is then discarded. Old-PID Words/shared sends release ownership without reaching the replacement. Both the replacement and survivor read valid shared bytes after old normal exit or contained failure.

Plain cancellation of the retired monitor leaves its queued Down intact and the new monitor usable. Final normal close checks the current member's own Down. Explicit room death and active shutdown release three old events plus nineteen queued shares in three cleanup batches; the host retains valid bytes until the final reference is dropped once.

Source review found no blocking oracle gap. During development the checked boundary rejected host-only demonitor_info and a bool use of the unit-returning demonitor; the fixture now uses admitted plain demonitor, with queue and relationship observations rather than a return-value claim. The first retained cohort passed. Eighteen runtime/compiler files match the previous milestone after crate renaming. No speed test or performance comparison ran; earlier sources and evidence remain preserved.

Application epochs are trusted protocol labels, not authenticated senders. Old Down publication occurs before replacement; only consumption is deferred. This is a bounded integration fixture, not general membership, automatic restart supervision or complete G1–G5 qualification. Next: checked self-exit and targeted local exit with restricted reason values, trapping, exact notifications and cleanup.
