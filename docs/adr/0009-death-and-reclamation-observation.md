---
status: accepted
---

# Observe process death separately from final reclamation

The user selected `Down` to mean that process execution has stopped and its lifetime is no longer addressable, while completion of tracked cleanup can be observed separately. This preserves useful death observation without waiting for every reclamation step, but does not adopt BEAM's dirty-NIF exception that lets native process work continue after logical death. Cleanup must release the dead process's owned references and resources without invalidating shared immutable storage still owned by live recipients; exact resource invalidation and observation APIs remain to be qualified.
