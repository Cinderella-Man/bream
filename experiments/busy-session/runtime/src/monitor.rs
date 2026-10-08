use super::*;
use std::collections::{BTreeMap, BTreeSet};

/// Opaque observer-scoped identity. A copied reference owns no process storage.
/// Dropping a reference does not cancel its registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonitorRef {
    owner: ProcessId,
    serial: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorError {
    ObserverNotLive,
    Exhausted,
}
/// A genuine runtime notification; application construction is not admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Down {
    reference: MonitorRef,
    target: ProcessId,
    reason: u64,
}
impl Down {
    pub fn reference(self) -> MonitorRef {
        self.reference
    }
    pub fn target(self) -> ProcessId {
        self.target
    }
    /// Restricted reasons: 1 normal completion, 2 forced stop, 3 process failure,
    /// 4 absent incarnation. These are prototype codes, not Erlang terms.
    pub fn reason_code(self) -> u64 {
        self.reason
    }
}
struct Entry {
    target: ProcessId,
    observer: Weak<Metrics>,
}
#[derive(Default)]
struct Table {
    next: u64,
    entries: BTreeMap<MonitorRef, Entry>,
    outgoing: BTreeMap<ProcessId, BTreeSet<MonitorRef>>,
    incoming: BTreeMap<ProcessId, BTreeSet<MonitorRef>>,
    absent: BTreeSet<MonitorRef>,
}
impl Table {
    fn unlink(
        index: &mut BTreeMap<ProcessId, BTreeSet<MonitorRef>>,
        id: ProcessId,
        reference: MonitorRef,
    ) {
        if let Some(refs) = index.get_mut(&id) {
            refs.remove(&reference);
            if refs.is_empty() {
                index.remove(&id);
            }
        }
    }
    fn remove(&mut self, reference: MonitorRef) -> Option<Entry> {
        let entry = self.entries.remove(&reference)?;
        self.absent.remove(&reference);
        Self::unlink(&mut self.outgoing, reference.owner, reference);
        Self::unlink(&mut self.incoming, entry.target, reference);
        Some(entry)
    }
    fn commit(&mut self, reference: MonitorRef, reason: u64) {
        // Cancellation and mailbox conversion serialize here. A detached
        // death notification has no authority once the origin is removed.
        let Some(entry) = self.remove(reference) else {
            return;
        };
        if let Some(observer) = entry.observer.upgrade() {
            mailbox::deliver_event(
                &observer,
                Event::Down(Down {
                    reference,
                    target: entry.target,
                    reason,
                }),
            );
        }
    }
}
#[derive(Default)]
pub(super) struct Hub {
    state: Mutex<Table>,
    wake_queued: AtomicBool,
}
impl Hub {
    pub(super) fn create(
        &self,
        owner: ProcessId,
        observer: &Arc<Metrics>,
        target: ProcessId,
        live: bool,
    ) -> Result<(MonitorRef, bool), MonitorError> {
        let mut table = self.state.lock().unwrap();
        let serial = table.next.checked_add(1).ok_or(MonitorError::Exhausted)?;
        table.next = serial;
        let reference = MonitorRef { owner, serial };
        // BEAM self-monitor allocates a fresh reference but no relationship.
        if owner == target {
            return Ok((reference, false));
        }
        table.entries.insert(
            reference,
            Entry {
                target,
                observer: Arc::downgrade(observer),
            },
        );
        table.outgoing.entry(owner).or_default().insert(reference);
        if live {
            table.incoming.entry(target).or_default().insert(reference);
        } else {
            table.absent.insert(reference);
        }
        Ok((reference, !live))
    }
    pub(super) fn queue_wake(&self) -> bool {
        !self.wake_queued.swap(true, Ordering::AcqRel)
    }
    pub(super) fn acknowledge_wake(&self) {
        self.wake_queued.store(false, Ordering::Release);
    }
    #[cfg(test)]
    pub(super) fn exhaust_references(&self) {
        self.state.lock().unwrap().next = u64::MAX;
    }
    pub(super) fn cancel(&self, owner: ProcessId, reference: MonitorRef) -> bool {
        if reference.owner != owner {
            return false;
        }
        self.state.lock().unwrap().remove(reference).is_some()
    }
    pub(super) fn active(&self) -> usize {
        self.state.lock().unwrap().entries.len()
    }
    pub(super) fn has_pending(&self) -> bool {
        !self.state.lock().unwrap().absent.is_empty()
    }
    pub(super) fn service(&self, limit: usize) -> usize {
        let mut table = self.state.lock().unwrap();
        let mut count = 0;
        while count < limit {
            let Some(reference) = table.absent.pop_first() else {
                break;
            };
            table.commit(reference, 4);
            count += 1;
        }
        count
    }
    pub(super) fn detach(self: &Arc<Self>, id: ProcessId, reason: u8) -> Retiring {
        let mut table = self.state.lock().unwrap();
        Retiring {
            hub: self.clone(),
            incoming: table.incoming.remove(&id).unwrap_or_default(),
            outgoing: table.outgoing.remove(&id).unwrap_or_default(),
            reason: u64::from(reason),
        }
    }
}
/// Detached indices own only references. Each cleanup turn settles a bounded
/// number; stopping never walks an arbitrary relationship set under its lock.
pub(super) struct Retiring {
    hub: Arc<Hub>,
    incoming: BTreeSet<MonitorRef>,
    outgoing: BTreeSet<MonitorRef>,
    reason: u64,
}
impl Retiring {
    pub(super) fn step(&mut self, limit: usize) -> bool {
        let mut table = self.hub.state.lock().unwrap();
        for _ in 0..limit {
            if let Some(reference) = self.outgoing.pop_first() {
                table.remove(reference);
            } else if let Some(reference) = self.incoming.pop_first() {
                table.commit(reference, self.reason);
            } else {
                break;
            }
        }
        self.incoming.is_empty() && self.outgoing.is_empty()
    }
}
