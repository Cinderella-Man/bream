use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkError {
    OriginNotLive,
    NoProcess,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkResult {
    Linked,
    NoProcess,
}
/// A genuine converted linked-exit notification; application code cannot forge it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exit {
    pub(super) from: ProcessId,
    pub(super) reason: u64,
}
impl Exit {
    pub fn from(self) -> ProcessId {
        self.from
    }
    pub fn reason_code(self) -> u64 {
        self.reason
    }
}
type Pair = (ProcessId, ProcessId);
fn pair(a: ProcessId, b: ProcessId) -> Pair {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}
struct Edge {
    low: Weak<Metrics>,
    high: Weak<Metrics>,
}
impl Edge {
    fn peer(&self, key: Pair, from: ProcessId) -> &Weak<Metrics> {
        if from == key.0 {
            &self.high
        } else {
            &self.low
        }
    }
}
#[derive(Default)]
struct Table {
    edges: BTreeMap<Pair, Edge>,
    index: BTreeMap<ProcessId, BTreeSet<Pair>>,
    absent: BTreeMap<Pair, ProcessId>, // pair -> missing sender
}
impl Table {
    fn remove(&mut self, key: Pair) -> Option<Edge> {
        let edge = self.edges.remove(&key)?;
        self.absent.remove(&key);
        for id in [key.0, key.1] {
            if let Some(set) = self.index.get_mut(&id) {
                set.remove(&key);
                if set.is_empty() {
                    self.index.remove(&id);
                }
            }
        }
        Some(edge)
    }
    fn convert(&mut self, key: Pair, from: ProcessId, reason: u64) {
        // This is deliberately keyed by current peer identity, not a link
        // generation. Re-establishment can make an older linked exit applicable.
        let Some(edge) = self.remove(key) else {
            return;
        };
        let Some(receiver) = edge.peer(key, from).upgrade() else {
            return;
        };
        exit::deliver(&receiver, from, reason, exit::Delivery::Linked);
        // A normal untrapped exit is ignored, but its topology is still gone.
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
        origin: ProcessId,
        owner: &Arc<Metrics>,
        target: ProcessId,
        peer: Option<&Arc<Metrics>>,
    ) -> Result<bool, LinkError> {
        let mut table = self.state.lock().unwrap();
        if origin == target {
            return Ok(false);
        }
        // Pinned BEAM checks absent + non-trapping before its idempotent branch.
        if peer.is_none() && !owner.trap_exit.load(Ordering::Acquire) {
            return Err(LinkError::NoProcess);
        }
        let key = pair(origin, target);
        if table.edges.contains_key(&key) {
            return Ok(false);
        }
        let owner = Arc::downgrade(owner);
        let peer = peer.map(Arc::downgrade).unwrap_or_default();
        let edge = if origin == key.0 {
            Edge {
                low: owner,
                high: peer,
            }
        } else {
            Edge {
                low: peer,
                high: owner,
            }
        };
        let missing = edge.peer(key, origin).strong_count() == 0;
        table.edges.insert(key, edge);
        table.index.entry(origin).or_default().insert(key);
        if missing {
            table.absent.insert(key, target);
        } else {
            table.index.entry(target).or_default().insert(key);
        }
        Ok(missing)
    }
    pub(super) fn unlink(&self, origin: ProcessId, target: ProcessId) {
        self.state.lock().unwrap().remove(pair(origin, target));
    }
    pub(super) fn set_trap(&self, metrics: &Metrics, enabled: bool) -> bool {
        let _conversion = self.state.lock().unwrap();
        metrics.trap_exit.swap(enabled, Ordering::AcqRel)
    }
    pub(super) fn active(&self) -> usize {
        self.state.lock().unwrap().edges.len()
    }
    pub(super) fn has_pending(&self) -> bool {
        !self.state.lock().unwrap().absent.is_empty()
    }
    pub(super) fn queue_wake(&self) -> bool {
        !self.wake_queued.swap(true, Ordering::AcqRel)
    }
    pub(super) fn acknowledge_wake(&self) {
        self.wake_queued.store(false, Ordering::Release);
    }
    pub(super) fn service(&self, limit: usize) -> usize {
        let mut table = self.state.lock().unwrap();
        let mut count = 0;
        while count < limit {
            let Some((key, from)) = table.absent.pop_first() else {
                break;
            };
            table.convert(key, from, 4);
            count += 1;
        }
        count
    }
    pub(super) fn detach(self: &Arc<Self>, owner: ProcessId, reason: u64) -> Retiring {
        let keys = self
            .state
            .lock()
            .unwrap()
            .index
            .remove(&owner)
            .unwrap_or_default();
        Retiring {
            hub: self.clone(),
            keys,
            owner,
            reason,
        }
    }
}
pub(super) struct Retiring {
    hub: Arc<Hub>,
    keys: BTreeSet<Pair>,
    owner: ProcessId,
    reason: u64,
}
impl Retiring {
    pub(super) fn step(&mut self, limit: usize) -> (bool, usize) {
        let mut table = self.hub.state.lock().unwrap();
        let mut used = 0;
        while used < limit {
            let Some(key) = self.keys.pop_first() else {
                break;
            };
            table.convert(key, self.owner, self.reason);
            used += 1;
        }
        (self.keys.is_empty(), used)
    }
}
