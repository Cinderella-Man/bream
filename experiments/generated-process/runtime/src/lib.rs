//! Internal machinery for a research executable, not a public process-safe API.
//! Context and futures are intentionally ordinary Rust types. Only the audited
//! macro inputs in the executable are part of the demonstrated restricted route.
#![forbid(unsafe_code)]
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

#[derive(Default)]
pub struct Metrics {
    pub progress: AtomicU64,
    pub live_bytes: AtomicU64,
    pub allocated_bytes: AtomicU64,
    pub freed_bytes: AtomicU64,
    pub drops: AtomicU64,
    pub drops_during_poll: AtomicU64,
}

pub struct ProcessContext {
    metrics: Arc<Metrics>,
    polling: Arc<AtomicBool>,
}
impl ProcessContext {
    pub fn new(metrics: Arc<Metrics>, polling: Arc<AtomicBool>) -> Self {
        Self { metrics, polling }
    }
    pub fn observe(&self, value: u64) {
        self.metrics.progress.store(value, Ordering::SeqCst);
    }
    pub fn owned_buffer(&self, bytes: u64) -> OwnedBuffer {
        // Fixed research admission policy; no guest-defined allocator or Drop.
        assert!(bytes <= 4096, "research buffer bound exceeded");
        let current = self.metrics.live_bytes.load(Ordering::SeqCst);
        assert!(current + bytes <= 65536, "research process budget exceeded");
        let storage = vec![0u8; bytes as usize];
        self.metrics.live_bytes.fetch_add(bytes, Ordering::SeqCst);
        self.metrics
            .allocated_bytes
            .fetch_add(bytes, Ordering::SeqCst);
        OwnedBuffer {
            storage,
            metrics: self.metrics.clone(),
            polling: self.polling.clone(),
        }
    }
}

pub struct OwnedBuffer {
    storage: Vec<u8>,
    metrics: Arc<Metrics>,
    polling: Arc<AtomicBool>,
}
impl Drop for OwnedBuffer {
    fn drop(&mut self) {
        let bytes = self.storage.len() as u64;
        self.metrics.live_bytes.fetch_sub(bytes, Ordering::SeqCst);
        self.metrics.freed_bytes.fetch_add(bytes, Ordering::SeqCst);
        self.metrics.drops.fetch_add(1, Ordering::SeqCst);
        if self.polling.load(Ordering::SeqCst) {
            self.metrics
                .drops_during_poll
                .fetch_add(1, Ordering::SeqCst);
        }
        // Vec storage is dropped normally after this bounded bookkeeping.
    }
}

pub struct Checkpoint {
    suspended: bool,
}
pub fn checkpoint() -> Checkpoint {
    Checkpoint { suspended: false }
}
impl Future for Checkpoint {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if self.suspended {
            Poll::Ready(())
        } else {
            self.suspended = true;
            context.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
