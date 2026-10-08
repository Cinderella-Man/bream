# Busy room member, shared bytes and termination

Passed for the fixed-membership single-worker profile, 2026-10-08. [Implementation and limits](README.md), [cohort](../../evidence/busy-session/runs/20261008-busy-01/results.json), [audit](../../evidence/busy-session/validation.json).

All 381 expected command outcomes passed, with 346 observations and 186 retained inputs. Each compiler profile includes 95 inherited unit tests, eight public control families at allowances 1/4/32, 148 expected compiler rejections and eight positive shadowing executions. The six busy-member configurations cover eighteen direct-stop, fatal-link and active-shutdown scenarios.

The room and healthy member complete request/reply work while a real broadcast recipient computes continuously. Its loop has no application receive, send, yield or stop check. Direct kill and linked exit stop it with receive count still one; the room checks the exact Down identity/reference/reason. The surviving member then answers two further requests with valid shared bytes. Twenty-two queued messages release in three cleanup batches, and application state remains frozen after stop.

The busy member's held share plus nineteen queued shares drop independently of the room, healthy member and host. Final cleanup leaves only the host reference, which reads all bytes before final release. Shared reference creation remains 23 through repeated helper calls: owning Work/Reader state moves without extra clones. This is a structural ownership result, not measured speed evidence.

Source review added an explicit stopped-computation checksum assertion. During development the checked grammar rejected division; the scratch transform now uses admitted addition/subtraction. No runtime or compiler behavior changed; eighteen files match the previous milestone after crate renaming. The first retained cohort passed. No benchmark or performance comparison ran, and prior sources/evidence remain preserved.

G1–G5 remain partial. Next: replace a stopped session in a bounded membership slot, isolate stale identities/messages/notifications from the replacement, and settle retained payloads across replacement and shutdown. General dynamic membership, richer exit reasons, direct checked exit APIs, authored byte construction and real-time timers remain open.
