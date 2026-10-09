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

/// Application values and genuine runtime notifications share one queue order.
pub enum Event {
    Message(Envelope),
    Shared(SharedEnvelope),
    Down(Down),
    Exit(Exit),
}
pub enum EventResult {
    Event(Event),
    Timeout,
}
pub enum DownResult {
    Down(Down),
    Timeout,
}
pub enum ExitResult {
    Exit(Exit),
    Timeout,
}
#[derive(Clone, Copy)]
enum Selector {
    Message(Option<u64>),
    Shared(Option<u64>),
    Any,
    Down(MonitorRef),
    Exit(ProcessId),
}
impl Selector {
    fn matches(self, event: &Event) -> bool {
        match (self, event) {
            (Self::Any, _) => true,
            (Self::Message(tag), Event::Message(message)) => {
                tag.is_none_or(|tag| tag == message.tag)
            }
            (Self::Shared(tag), Event::Shared(message)) => {
                tag.is_none_or(|tag| tag == message.tag())
            }
            (Self::Down(reference), Event::Down(down)) => reference == down.reference(),
            (Self::Exit(from), Event::Exit(exit)) => from == exit.from(),
            _ => false,
        }
    }
}
#[derive(Default)]
pub(super) struct Mailbox {
    pub(super) incoming: LinkedList<Event>,
    pub(super) saved: LinkedList<Event>,
    pub(super) waiting: bool,
    pub(super) closed: bool,
    pub(super) timer: Option<timer::Registration>,
    pub(super) next_timer: u64,
}
impl Mailbox {
    pub(super) fn reset_scan(&mut self) {
        self.saved.append(&mut self.incoming);
        std::mem::swap(&mut self.saved, &mut self.incoming);
    }
    pub(super) fn close(&mut self) -> LinkedList<Event> {
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
    deliver_event(metrics, Event::Message(envelope));
}
pub(super) fn deliver_event(metrics: &Metrics, envelope: Event) {
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
        match poll_receive(
            &self.metrics,
            &self.budget,
            Selector::Message(self.selector),
            None,
        ) {
            Poll::Ready(EventResult::Event(Event::Message(message))) => Poll::Ready(message),
            Poll::Ready(_) => unreachable!("infinite application receive"),
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
        match poll_receive(
            &self.receive.metrics,
            &self.receive.budget,
            Selector::Message(self.receive.selector),
            Some(self.ticks),
        ) {
            Poll::Ready(EventResult::Event(Event::Message(message))) => {
                Poll::Ready(ReceiveResult::Message(message))
            }
            Poll::Ready(EventResult::Timeout) => Poll::Ready(ReceiveResult::Timeout),
            Poll::Ready(_) => unreachable!("application selector"),
            Poll::Pending => Poll::Pending,
        }
    }
}
fn timed_out(inbox: &mut Mailbox, metrics: &Metrics) -> Poll<EventResult> {
    inbox.timer = None;
    inbox.reset_scan();
    inbox.waiting = false;
    metrics.timeouts.fetch_add(1, Ordering::Relaxed);
    Poll::Ready(EventResult::Timeout)
}
fn poll_receive(
    metrics: &Arc<Metrics>,
    budget: &Budget,
    selector: Selector,
    ticks: Option<u64>,
) -> Poll<EventResult> {
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
        if selector.matches(front) {
            let envelope = inbox.incoming.pop_front().unwrap();
            inbox.timer = None;
            inbox.reset_scan();
            inbox.waiting = false;
            metrics.receives.fetch_add(1, Ordering::Relaxed);
            return Poll::Ready(EventResult::Event(envelope));
        }
        // Transfer the existing front node; no clone, payload copy or
        // fresh list node per rejected candidate.
        let mut head = inbox.incoming.split_off(1);
        std::mem::swap(&mut head, &mut inbox.incoming);
        inbox.saved.append(&mut head);
    }
}

pub struct EventReceive {
    metrics: Arc<Metrics>,
    budget: Arc<Budget>,
}
impl EventReceive {
    pub(super) fn new(metrics: Arc<Metrics>, budget: Arc<Budget>) -> Self {
        Self { metrics, budget }
    }
}
impl Future for EventReceive {
    type Output = Event;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Event> {
        match poll_receive(&self.metrics, &self.budget, Selector::Any, None) {
            Poll::Ready(EventResult::Event(event)) => Poll::Ready(event),
            Poll::Ready(EventResult::Timeout) => unreachable!("infinite event receive"),
            Poll::Pending => Poll::Pending,
        }
    }
}
pub struct TimedEventReceive {
    receive: EventReceive,
    ticks: u64,
}
impl TimedEventReceive {
    pub(super) fn new(metrics: Arc<Metrics>, budget: Arc<Budget>, ticks: u64) -> Self {
        Self {
            receive: EventReceive::new(metrics, budget),
            ticks,
        }
    }
}
impl Future for TimedEventReceive {
    type Output = EventResult;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<EventResult> {
        poll_receive(
            &self.receive.metrics,
            &self.receive.budget,
            Selector::Any,
            Some(self.ticks),
        )
    }
}
pub struct DownReceive {
    receive: EventReceive,
    reference: MonitorRef,
}
impl DownReceive {
    pub(super) fn new(metrics: Arc<Metrics>, budget: Arc<Budget>, reference: MonitorRef) -> Self {
        Self {
            receive: EventReceive::new(metrics, budget),
            reference,
        }
    }
}
impl Future for DownReceive {
    type Output = Down;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Down> {
        match poll_receive(
            &self.receive.metrics,
            &self.receive.budget,
            Selector::Down(self.reference),
            None,
        ) {
            Poll::Ready(EventResult::Event(Event::Down(down))) => Poll::Ready(down),
            Poll::Ready(_) => unreachable!("infinite Down selector"),
            Poll::Pending => Poll::Pending,
        }
    }
}
pub struct TimedDownReceive {
    receive: DownReceive,
    ticks: u64,
}
impl TimedDownReceive {
    pub(super) fn new(
        metrics: Arc<Metrics>,
        budget: Arc<Budget>,
        reference: MonitorRef,
        ticks: u64,
    ) -> Self {
        Self {
            receive: DownReceive::new(metrics, budget, reference),
            ticks,
        }
    }
}
impl Future for TimedDownReceive {
    type Output = DownResult;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<DownResult> {
        match poll_receive(
            &self.receive.receive.metrics,
            &self.receive.receive.budget,
            Selector::Down(self.receive.reference),
            Some(self.ticks),
        ) {
            Poll::Ready(EventResult::Event(Event::Down(down))) => {
                Poll::Ready(DownResult::Down(down))
            }
            Poll::Ready(EventResult::Timeout) => Poll::Ready(DownResult::Timeout),
            Poll::Ready(_) => unreachable!("Down selector"),
            Poll::Pending => Poll::Pending,
        }
    }
}
/// Cancellation precedes construction. No subsequent conversion can add a
/// genuine Down for this reference. Scan work consumes the process allowance.
pub struct Flush {
    receive: DownReceive,
    started: bool,
}
impl Flush {
    pub(super) fn new(metrics: Arc<Metrics>, budget: Arc<Budget>, reference: MonitorRef) -> Self {
        Self {
            receive: DownReceive::new(metrics, budget, reference),
            started: false,
        }
    }
}
impl Future for Flush {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        if !self.started {
            self.receive
                .receive
                .metrics
                .mailbox
                .lock()
                .unwrap()
                .reset_scan();
            self.started = true;
        }
        match poll_receive(
            &self.receive.receive.metrics,
            &self.receive.receive.budget,
            Selector::Down(self.receive.reference),
            Some(0),
        ) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(_) => Poll::Ready(()),
        }
    }
}

pub struct ExitReceive {
    receive: EventReceive,
    from: ProcessId,
}
impl ExitReceive {
    pub(super) fn new(metrics: Arc<Metrics>, budget: Arc<Budget>, from: ProcessId) -> Self {
        Self {
            receive: EventReceive::new(metrics, budget),
            from,
        }
    }
}
impl Future for ExitReceive {
    type Output = Exit;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Exit> {
        match poll_receive(
            &self.receive.metrics,
            &self.receive.budget,
            Selector::Exit(self.from),
            None,
        ) {
            Poll::Ready(EventResult::Event(Event::Exit(exit))) => Poll::Ready(exit),
            Poll::Ready(_) => unreachable!("infinite Exit selector"),
            Poll::Pending => Poll::Pending,
        }
    }
}
pub struct TimedExitReceive {
    receive: ExitReceive,
    ticks: u64,
}
impl TimedExitReceive {
    pub(super) fn new(
        metrics: Arc<Metrics>,
        budget: Arc<Budget>,
        from: ProcessId,
        ticks: u64,
    ) -> Self {
        Self {
            receive: ExitReceive::new(metrics, budget, from),
            ticks,
        }
    }
}
impl Future for TimedExitReceive {
    type Output = ExitResult;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<ExitResult> {
        match poll_receive(
            &self.receive.receive.metrics,
            &self.receive.receive.budget,
            Selector::Exit(self.receive.from),
            Some(self.ticks),
        ) {
            Poll::Ready(EventResult::Event(Event::Exit(exit))) => {
                Poll::Ready(ExitResult::Exit(exit))
            }
            Poll::Ready(EventResult::Timeout) => Poll::Ready(ExitResult::Timeout),
            Poll::Ready(_) => unreachable!("Exit selector"),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct SharedReceive {
    metrics: Arc<Metrics>,
    budget: Arc<Budget>,
    selector: Option<u64>,
}
impl SharedReceive {
    pub(super) fn new(metrics: Arc<Metrics>, budget: Arc<Budget>, selector: Option<u64>) -> Self {
        Self {
            metrics,
            budget,
            selector,
        }
    }
}
impl Future for SharedReceive {
    type Output = SharedEnvelope;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        match poll_receive(
            &self.metrics,
            &self.budget,
            Selector::Shared(self.selector),
            None,
        ) {
            Poll::Ready(EventResult::Event(Event::Shared(message))) => Poll::Ready(message),
            Poll::Pending => Poll::Pending,
            Poll::Ready(_) => unreachable!("infinite shared selector"),
        }
    }
}
pub enum SharedReceiveResult {
    Message(SharedEnvelope),
    Timeout,
}
pub struct TimedSharedReceive {
    receive: SharedReceive,
    ticks: u64,
}
impl TimedSharedReceive {
    pub(super) fn new(
        metrics: Arc<Metrics>,
        budget: Arc<Budget>,
        selector: Option<u64>,
        ticks: u64,
    ) -> Self {
        Self {
            receive: SharedReceive::new(metrics, budget, selector),
            ticks,
        }
    }
}
impl Future for TimedSharedReceive {
    type Output = SharedReceiveResult;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        match poll_receive(
            &self.receive.metrics,
            &self.receive.budget,
            Selector::Shared(self.receive.selector),
            Some(self.ticks),
        ) {
            Poll::Ready(EventResult::Event(Event::Shared(message))) => {
                Poll::Ready(SharedReceiveResult::Message(message))
            }
            Poll::Ready(EventResult::Timeout) => Poll::Ready(SharedReceiveResult::Timeout),
            Poll::Pending => Poll::Pending,
            Poll::Ready(_) => unreachable!("timed shared selector"),
        }
    }
}
