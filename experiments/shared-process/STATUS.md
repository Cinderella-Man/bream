# Immutable byte sharing and bounded broadcast

Passed for the sealed-byte, fixed-membership, single-worker profile, 2026-10-08. [Implementation and limits](README.md), [cohort](../../evidence/shared-process/runs/20261008-shared-01/results.json), [audit](../../evidence/shared-process/validation.json).

All 375 expected command outcomes passed, with 340 observations and 185 retained inputs. Each compiler profile passes 95 runtime tests, seven families of controls at allowances 1/4/32, 148 expected compiler rejections and eight positive shadowing executions. Six sharing configurations cover sixty bounded lifetime/broadcast scenarios and 96 shared-send/kill races.

Zero/one/two-recipient broadcast shares one immutable backing allocation through explicit owned references. Checked checksum reads succeed after publisher death or another receiver's death. Receiver failure stays isolated; exact Down follows that child's acknowledgement where one is expected. Partial fanout and a suspended send operand cancel without publishing extra shares. Host-held references outlive process shutdown; final release occurs once, independently of process stop. Nineteen queued shared messages retire in three batches.

Review tightened per-child acknowledgement ordering and separated partial-fanout reference accounting from post-death checksum observations. No runtime defect was found in the final source review. During development, the checked application rejected unsupported `-=` syntax; the fixture now uses admitted subtraction and assignment. The retained qualification passed on its first cohort.

No benchmark or speed comparison ran. Moving a Vec descriptor into private Arc storage preserves its byte allocation; this is a source/identity result, not a speed claim. Prior experiment sources and correctness evidence remain unchanged. G1–G5 remain partial. Next: make a session itself run sustained admitted computation while the room and another session progress, then stop it with shared payloads retained. General membership, direct exit APIs, arbitrary reasons, authored byte construction and real-time timers remain open.
