# Checked self-exit and targeted local exit

Passed for the declared local single-worker profile, 2026-10-09. [Implementation and limits](README.md), [cohort](../../evidence/explicit-exit/runs/20261009-exit-01/results.json), [audit](../../evidence/explicit-exit/validation.json).

All 424 expected command outcomes passed, retaining 382 aggregate observations and 205 inputs. Debug and release each pass 105 runtime tests, 159 expected compiler rejections and nine positive shadowing examples. Six public exit configurations match the thirty-case BEAM matrix. The JIT and debug emulator each pass thirty-nine reference observations.

Checked self_exit preserves the exact reason and never returns into application code. Targeted exit respects trapping, the legacy self-normal rule, the Kill/Killed distinction and full process identity. Busy targets stop without another receive; stale-PID Kill cannot reach a replacement. Held buffers and queued shared ownership settle once. An inherited failed-link path that could overwrite a prior fatal reason was fixed and regression-tested.

Direct local signal receipt is eager, not an implementation of OTP's queued direct-signal backlog. Application catch, arbitrary reasons/native code and multiple workers remain outside scope. G1–G5 remain partial. No speed tests or performance comparisons ran; prior milestone inputs remain preserved.

Next: a bounded runtime-owned external read adapter, separating actor death, operation completion and final storage release, with exact-incarnation delivery and explicit outstanding-resource accounting.
