---
status: accepted
---

# Typed protocols with growing mailboxes and selective receive

The first primitive experiments use typed Rust message protocols, while process identity, monitoring and linking remain independent of the payload type. The user chose ordinary local sends that do not wait for receiver consumption and mailboxes that can grow, prioritizing familiar BEAM semantics over bounded admission errors; experiments must measure queue growth without claiming unlimited memory. The user also selected selective receive with an optional timeout: consume the earliest matching message and retain unmatched messages; bounded admission can be a separate later policy, and matching scans must remain automatically schedulable.
