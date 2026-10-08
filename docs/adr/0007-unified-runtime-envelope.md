---
status: accepted
---

# One typed mailbox envelope for messages and notifications

Application protocol values, monitor-down notifications and trapped exits participate in one selective mailbox through a runtime-owned envelope, keeping lifecycle variants out of each application protocol enum. The user selected this shape after reviewing BEAM's conversion of eligible signals into mailbox messages; separate public queues would complicate or change cross-kind ordering and selection. Internal control signals remain distinct and do not wait for application receive; exact Rust names and signatures remain experimental.
