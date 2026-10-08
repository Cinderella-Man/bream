---
status: accepted
---

# BEAM-style exits and trapping

The user selected BEAM-style link and exit semantics for the first primitive experiments: normal exits generally leave linked peers alive, abnormal exits propagate, and eligible exit signals become mailbox notifications when the receiver traps exits. Direct forced kill remains untrappable; the reason later propagated to linked processes must follow the reference rules rather than turn every propagated reason into an untrappable kill. Monitoring remains one-way observation, and neither links nor monitors imply supervision or automatic restart.
