use super::*;

/// Immutable byte storage with a separate owned reference for each holder.
/// The host transfers the Vec allocation at ingestion; no byte mutation or
/// conversion back to mutable storage is exposed.
pub struct SharedBytes {
    storage: Arc<Storage>,
}
struct Storage {
    bytes: Vec<u8>,
    counters: Arc<Counters>,
}
#[derive(Default)]
struct Counters {
    live_bytes: AtomicU64,
    freed_bytes: AtomicU64,
    physical_drops: AtomicU64,
    live_references: AtomicU64,
    references_created: AtomicU64,
    references_dropped: AtomicU64,
}
/// This diagnostic receipt owns counters only, never a payload reference.
#[derive(Clone)]
pub struct SharedReceipt {
    counters: Arc<Counters>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedSnapshot {
    pub live_bytes: u64,
    pub freed_bytes: u64,
    pub physical_drops: u64,
    pub live_references: u64,
    pub references_created: u64,
    pub references_dropped: u64,
}
impl SharedReceipt {
    /// Independent counters, not an atomic multi-field snapshot. Exact balance
    /// checks require a quiescent ownership boundary such as completed cleanup.
    pub fn snapshot(&self) -> SharedSnapshot {
        let c = &self.counters;
        let physical_drops = c.physical_drops.load(Ordering::Acquire);
        SharedSnapshot {
            physical_drops,
            live_bytes: c.live_bytes.load(Ordering::Acquire),
            freed_bytes: c.freed_bytes.load(Ordering::Acquire),
            live_references: c.live_references.load(Ordering::Acquire),
            references_created: c.references_created.load(Ordering::Acquire),
            references_dropped: c.references_dropped.load(Ordering::Acquire),
        }
    }
}
impl SharedBytes {
    pub fn new(bytes: Vec<u8>) -> Result<(Self, SharedReceipt), &'static str> {
        if bytes.capacity() > 65536 {
            return Err("shared payload exceeds experimental 65536-byte capacity limit");
        }
        let counters = Arc::new(Counters::default());
        counters
            .live_bytes
            .store(bytes.capacity() as u64, Ordering::Relaxed);
        counters.live_references.store(1, Ordering::Relaxed);
        counters.references_created.store(1, Ordering::Relaxed);
        let receipt = SharedReceipt {
            counters: counters.clone(),
        };
        Ok((
            Self {
                storage: Arc::new(Storage { bytes, counters }),
            },
            receipt,
        ))
    }
    pub fn len(&self) -> u64 {
        self.storage.bytes.len() as u64
    }
    pub fn is_empty(&self) -> bool {
        self.storage.bytes.is_empty()
    }
    pub fn read(&self, index: u64) -> u64 {
        u64::from(self.storage.bytes[index as usize])
    }
}
impl Clone for SharedBytes {
    fn clone(&self) -> Self {
        let storage = self.storage.clone();
        storage
            .counters
            .references_created
            .fetch_add(1, Ordering::Relaxed);
        storage
            .counters
            .live_references
            .fetch_add(1, Ordering::Release);
        Self { storage }
    }
}
impl Drop for SharedBytes {
    fn drop(&mut self) {
        self.storage
            .counters
            .live_references
            .fetch_sub(1, Ordering::Relaxed);
        self.storage
            .counters
            .references_dropped
            .fetch_add(1, Ordering::Release);
        // Arc drops storage after this handle's accounting, when it is last.
    }
}
impl Drop for Storage {
    fn drop(&mut self) {
        let capacity = self.bytes.capacity() as u64;
        drop(std::mem::take(&mut self.bytes));
        self.counters.live_bytes.store(0, Ordering::Relaxed);
        self.counters.freed_bytes.store(capacity, Ordering::Relaxed);
        self.counters.physical_drops.fetch_add(1, Ordering::Release);
    }
}
/// Metadata is owned per message; only the sealed immutable payload is shared.
pub struct SharedEnvelope {
    tag: u64,
    bytes: SharedBytes,
}
impl SharedEnvelope {
    pub fn new(tag: u64, bytes: SharedBytes) -> Self {
        Self { tag, bytes }
    }
    pub fn tag(&self) -> u64 {
        self.tag
    }
    pub fn into_bytes(self) -> SharedBytes {
        self.bytes
    }
}

#[cfg(test)]
mod tests;
