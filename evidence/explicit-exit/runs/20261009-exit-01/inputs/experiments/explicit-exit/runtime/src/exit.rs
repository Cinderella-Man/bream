//! Restricted local exit reasons and receipt-time signal handling.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitReason {
    Normal,
    Killed,
    Failure,
    NoProcess,
    Kill,
}
impl ExitReason {
    pub fn code(self) -> u64 {
        match self {
            Self::Normal => 1,
            Self::Killed => 2,
            Self::Failure => 3,
            Self::NoProcess => 4,
            Self::Kill => 5,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitError {
    OriginNotLive,
}
#[derive(Clone, Copy)]
pub(super) enum Delivery {
    Direct,
    Linked,
}

pub(super) fn deliver(receiver: &Arc<Metrics>, from: ProcessId, reason: u64, mode: Delivery) {
    // Serialize fatal receipts with host kill and terminal commitment. A first
    // fatal receipt is irreversible; later signals cannot rewrite its reason.
    let _claim = receiver.lifecycle.lock().unwrap();
    if receiver.terminal.load(Ordering::Acquire) != 0
        || receiver.kill_event.load(Ordering::Acquire) != 0
        || receiver.pending_exit.load(Ordering::Acquire) != 0
    {
        return;
    }
    let direct = matches!(mode, Delivery::Direct);
    if direct && reason == ExitReason::Kill.code() {
        receiver.kill_event.store(event(), Ordering::Release);
        mailbox::notify(receiver);
    } else if receiver.trap_exit.load(Ordering::Acquire) {
        mailbox::deliver_event(receiver, Event::Exit(Exit { from, reason }));
    } else if reason != ExitReason::Normal.code() || (direct && receiver.id.get() == Some(&from)) {
        receiver.pending_exit.store(reason, Ordering::Release);
        mailbox::notify(receiver);
    }
}

pub(super) fn claim_local(metrics: &Metrics, reason: u64) {
    let _claim = metrics.lifecycle.lock().unwrap();
    if metrics.terminal.load(Ordering::Acquire) == 0
        && metrics.kill_event.load(Ordering::Acquire) == 0
        && metrics.pending_exit.load(Ordering::Acquire) == 0
    {
        metrics.pending_exit.store(reason, Ordering::Release);
    }
}

impl ProcessContext {
    /// Uncaught exit/1 for the checked profile. It has no application catch.
    /// Returning Pending transfers destruction back to the worker, outside poll.
    pub async fn self_exit(&self, reason: ExitReason) {
        claim_local(&self.metrics, reason.code());
        std::future::pending::<()>().await;
    }
    /// Local legacy exit/2, including non-returning fatal self-targets.
    pub async fn exit_to(&self, target: ProcessId, reason: ExitReason) {
        let shared = self
            .metrics
            .runtime
            .get()
            .and_then(Weak::upgrade)
            .expect("live runtime");
        (RuntimeClient { shared })
            .exit(self.self_id(), target, reason)
            .expect("live origin");
        if target == self.self_id()
            && (self.metrics.pending_exit.load(Ordering::Acquire) != 0
                || self.metrics.kill_event.load(Ordering::Acquire) != 0)
        {
            std::future::pending::<()>().await;
        }
    }
}
