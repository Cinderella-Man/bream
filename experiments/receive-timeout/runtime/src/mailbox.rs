use super::*;

/// One concrete admitted application protocol for this milestone.
/// No arbitrary user destructor or callback can be stored in this envelope.
pub struct Envelope {
    tag: u64,
    words: OwnedWords,
}
#[derive(Clone)]
pub struct MessageReceipt {
    allocation: Arc<Allocation>,
}
impl MessageReceipt {
    pub fn live_bytes(&self) -> u64 {
        self.allocation.live_bytes.load(Ordering::Acquire)
    }
    pub fn freed_bytes(&self) -> u64 {
        self.allocation.freed_bytes.load(Ordering::Acquire)
    }
    pub fn drops(&self) -> u64 {
        self.allocation.drops.load(Ordering::Acquire)
    }
}
impl Envelope {
    pub fn new(tag: u64, words: Vec<u64>) -> Result<(Self, MessageReceipt), &'static str> {
        if words.capacity() > 8192 {
            return Err("payload exceeds experimental 8192-word capacity limit");
        }
        let allocation = Arc::new(Allocation::default());
        allocation
            .live_bytes
            .store(words.capacity() as u64 * 8, Ordering::Relaxed);
        let receipt = MessageReceipt {
            allocation: allocation.clone(),
        };
        Ok((
            Self {
                tag,
                words: OwnedWords { words, allocation },
            },
            receipt,
        ))
    }
    pub(super) fn from_owned(tag: u64, words: OwnedWords) -> Self {
        Self { tag, words }
    }
    pub fn tag(&self) -> u64 {
        self.tag
    }
    pub fn into_words(self) -> OwnedWords {
        self.words
    }
}

#[derive(Default)]
pub(super) struct Mailbox {
    pub(super) incoming: LinkedList<Envelope>,
    pub(super) saved: LinkedList<Envelope>,
    pub(super) waiting: bool,
    pub(super) closed: bool,
    pub(super) timer: Option<timer::Registration>,
    pub(super) next_timer: u64,
}
impl Mailbox {
    fn reset_scan(&mut self) {
        self.saved.append(&mut self.incoming);
        std::mem::swap(&mut self.saved, &mut self.incoming);
    }
    pub(super) fn close(&mut self) -> LinkedList<Envelope> {
        self.closed = true;
        self.waiting = false;
        self.timer = None;
        self.reset_scan();
        std::mem::take(&mut self.incoming)
    }
}

// Waiting-state changes and message publication share the mailbox lock. The
// sole worker processes wake commands only outside polling/park commitment.
pub(super) fn notify(metrics: &Metrics) {
    let mut inbox = metrics.mailbox.lock().unwrap();
    if inbox.closed {
        return;
    }
    inbox.waiting = false;
    drop(inbox);
    if let (Some(id), Some(shared)) = (
        metrics.id.get(),
        metrics.runtime.get().and_then(Weak::upgrade),
    ) {
        shared.wake(*id);
    }
}
pub(super) fn deliver(metrics: &Metrics, envelope: Envelope) {
    let mut inbox = metrics.mailbox.lock().unwrap();
    if inbox.closed {
        drop(inbox);
        drop(envelope);
        return;
    }
    inbox.incoming.push_back(envelope);
    let wake = inbox.waiting;
    inbox.waiting = false;
    drop(inbox);
    if wake {
        if let (Some(id), Some(shared)) = (
            metrics.id.get(),
            metrics.runtime.get().and_then(Weak::upgrade),
        ) {
            shared.wake(*id);
        }
    }
}

pub struct Receive {
    metrics: Arc<Metrics>,
    budget: Arc<Budget>,
    selector: Option<u64>,
}
impl Receive {
    pub(super) fn new(metrics: Arc<Metrics>, budget: Arc<Budget>, selector: Option<u64>) -> Self {
        Self {
            metrics,
            budget,
            selector,
        }
    }
}
impl Future for Receive {
    type Output = Envelope;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Envelope> {
        match poll_receive(&self.metrics, &self.budget, self.selector, None) {
            Poll::Ready(ReceiveResult::Message(message)) => Poll::Ready(message),
            Poll::Ready(ReceiveResult::Timeout) => unreachable!("infinite receive expired"),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub enum ReceiveResult {
    Message(Envelope),
    Timeout,
}
pub struct TimedReceive {
    receive: Receive,
    ticks: u64,
}
impl TimedReceive {
    pub(super) fn new(
        metrics: Arc<Metrics>,
        budget: Arc<Budget>,
        selector: Option<u64>,
        ticks: u64,
    ) -> Self {
        Self {
            receive: Receive::new(metrics, budget, selector),
            ticks,
        }
    }
}
impl Future for TimedReceive {
    type Output = ReceiveResult;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<ReceiveResult> {
        poll_receive(
            &self.receive.metrics,
            &self.receive.budget,
            self.receive.selector,
            Some(self.ticks),
        )
    }
}
fn timed_out(inbox: &mut Mailbox, metrics: &Metrics) -> Poll<ReceiveResult> {
    inbox.timer = None;
    inbox.reset_scan();
    inbox.waiting = false;
    metrics.timeouts.fetch_add(1, Ordering::Relaxed);
    Poll::Ready(ReceiveResult::Timeout)
}
fn poll_receive(
    metrics: &Arc<Metrics>,
    budget: &Budget,
    selector: Option<u64>,
    ticks: Option<u64>,
) -> Poll<ReceiveResult> {
    let mut inbox = metrics.mailbox.lock().unwrap();
    assert!(!inbox.closed, "stopped process was polled");
    // Timer service runs between application polls. A committed expiry
    // selects timeout before resuming the scan, including already-ready work.
    if inbox.timer.as_ref().is_some_and(|timer| timer.expired) {
        return timed_out(&mut inbox, metrics);
    }
    loop {
        let Some(front) = inbox.incoming.front() else {
            if let Some(ticks) = ticks {
                if ticks == 0 {
                    return timed_out(&mut inbox, metrics);
                }
                if ticks > u64::from(u32::MAX) {
                    inbox.reset_scan();
                    drop(inbox);
                    panic!("receive timeout exceeds u32 range");
                }
                if inbox.timer.is_none() {
                    let Some(generation) = inbox.next_timer.checked_add(1) else {
                        inbox.reset_scan();
                        drop(inbox);
                        panic!("receive timer generation exhausted");
                    };
                    let shared = metrics
                        .runtime
                        .get()
                        .and_then(Weak::upgrade)
                        .expect("live timer runtime");
                    match shared.clock.arm(metrics, generation, ticks) {
                        Ok(timer) => {
                            inbox.next_timer = generation;
                            inbox.timer = Some(timer);
                        }
                        Err(error) => {
                            inbox.reset_scan();
                            drop(inbox);
                            panic!("receive timer failed: {error:?}");
                        }
                    }
                }
            }
            inbox.waiting = true;
            return Poll::Pending;
        };
        if !budget.charge() {
            inbox.waiting = false; // Unexamined input: yield, do not park.
            return Poll::Pending;
        }
        metrics.scans.fetch_add(1, Ordering::Relaxed);
        if selector.is_none_or(|tag| tag == front.tag) {
            let envelope = inbox.incoming.pop_front().unwrap();
            inbox.timer = None;
            inbox.reset_scan();
            inbox.waiting = false;
            metrics.receives.fetch_add(1, Ordering::Relaxed);
            return Poll::Ready(ReceiveResult::Message(envelope));
        }
        // Transfer the existing front node; no clone, payload copy or
        // fresh list node per rejected candidate.
        let mut head = inbox.incoming.split_off(1);
        std::mem::swap(&mut head, &mut inbox.incoming);
        inbox.saved.append(&mut head);
    }
}
