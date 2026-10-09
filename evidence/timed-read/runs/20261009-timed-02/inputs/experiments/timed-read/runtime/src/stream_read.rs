//! Persistent local endpoint; each bounded read has a separate request identity.
use super::*;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;

const LIMIT: usize = 16;
#[derive(Default)]
struct Counters {
    started: AtomicU64,
    settled: AtomicU64,
    delivered: AtomicU64,
    discarded: AtomicU64,
    attempts: AtomicU64,
    timeouts: AtomicU64,
    buffers: AtomicU64,
    drops: AtomicU64,
    live_bytes: AtomicU64,
    endpoint_drops: AtomicU64,
    active_id: AtomicU64,
    last_id: AtomicU64,
    retired: AtomicBool,
}
#[derive(Clone)]
pub struct StreamReceipt {
    counters: Arc<Counters>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamSnapshot {
    pub started: u64,
    pub settled: u64,
    pub delivered: u64,
    pub discarded: u64,
    pub attempts: u64,
    pub timeouts: u64,
    pub buffers: u64,
    pub drops: u64,
    pub live_bytes: u64,
    pub endpoint_drops: u64,
    pub active_id: u64,
    pub last_id: u64,
    pub retired: bool,
}
impl StreamReceipt {
    /// Independent counters. Compare balances only at controlled ownership boundaries.
    pub fn snapshot(&self) -> StreamSnapshot {
        let c = &self.counters;
        StreamSnapshot {
            retired: c.retired.load(Ordering::Acquire),
            started: c.started.load(Ordering::Acquire),
            settled: c.settled.load(Ordering::Acquire),
            delivered: c.delivered.load(Ordering::Acquire),
            discarded: c.discarded.load(Ordering::Acquire),
            attempts: c.attempts.load(Ordering::Acquire),
            timeouts: c.timeouts.load(Ordering::Acquire),
            buffers: c.buffers.load(Ordering::Acquire),
            drops: c.drops.load(Ordering::Acquire),
            live_bytes: c.live_bytes.load(Ordering::Acquire),
            endpoint_drops: c.endpoint_drops.load(Ordering::Acquire),
            active_id: c.active_id.load(Ordering::Acquire),
            last_id: c.last_id.load(Ordering::Acquire),
        }
    }
}
pub struct StreamBytes {
    bytes: Vec<u8>,
    counters: Arc<Counters>,
}
impl StreamBytes {
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
    pub fn read(&self, index: u64) -> u64 {
        u64::from(self.bytes[index as usize])
    }
}
impl Drop for StreamBytes {
    fn drop(&mut self) {
        let capacity = self.bytes.capacity() as u64;
        drop(std::mem::take(&mut self.bytes));
        self.counters
            .live_bytes
            .fetch_sub(capacity, Ordering::Relaxed);
        self.counters.drops.fetch_add(1, Ordering::Release);
    }
}
pub enum StreamResult {
    Data(StreamBytes),
    Eof,
    Failed,
    Unavailable,
    Busy,
}
pub enum TimedStreamResult {
    Data(StreamBytes),
    Eof,
    Failed,
    Unavailable,
    Busy,
    Timeout,
}
struct Endpoint {
    socket: UnixStream,
    counters: Arc<Counters>,
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        self.counters.endpoint_drops.fetch_add(1, Ordering::Release);
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    Open,
    Eof,
    Failed,
    Closed,
}
impl End {
    fn result(self) -> StreamResult {
        match self {
            Self::Eof => StreamResult::Eof,
            Self::Failed => StreamResult::Failed,
            _ => StreamResult::Unavailable,
        }
    }
}
enum Phase {
    Pending { bytes: StreamBytes, attempted: bool },
    Running,
    Ready(StreamResult),
    TimedOut,
}
#[derive(Default)]
struct Signals {
    cancelled: AtomicBool,
    expired: AtomicBool,
}
struct Active {
    id: u64,
    signals: Arc<Signals>,
    phase: Phase,
    timer: Option<timer::Registration>,
    zero: bool,
}
struct State {
    endpoint: Option<Endpoint>,
    next: Option<u64>,
    active: Option<Active>,
    end: End,
}
pub(super) struct Stream {
    owner: ProcessId,
    metrics: Weak<Metrics>,
    hub: Weak<Hub>,
    size: usize,
    closing: AtomicBool,
    counters: Arc<Counters>,
    state: Mutex<State>,
}
struct Request {
    id: u64,
    signals: Arc<Signals>,
    endpoint: Endpoint,
    bytes: StreamBytes,
}
enum Outcome {
    Pending(Request),
    Data(Endpoint, StreamBytes),
    Eof,
    Failed,
    Cancelled(Endpoint),
}
fn perform(mut request: Request, cancelled: bool) -> Outcome {
    if cancelled {
        return Outcome::Cancelled(request.endpoint);
    }
    request
        .bytes
        .counters
        .attempts
        .fetch_add(1, Ordering::Relaxed);
    match request.endpoint.socket.read(&mut request.bytes.bytes) {
        Ok(0) => Outcome::Eof,
        Ok(n) => {
            request.bytes.bytes.truncate(n);
            Outcome::Data(request.endpoint, request.bytes)
        }
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Outcome::Pending(request)
        }
        Err(_) => Outcome::Failed,
    }
}
impl Stream {
    fn wake(&self) {
        if let Some(hub) = self.hub.upgrade() {
            hub.wake();
        }
    }
    fn notify(&self) {
        if let Some(metrics) = self.metrics.upgrade() {
            if metrics.id.get() == Some(&self.owner) {
                mailbox::notify(&metrics);
            }
        }
    }
    fn close(&self) {
        self.closing.store(true, Ordering::Release);
        self.wake();
    }
    fn begin(self: &Arc<Self>) -> Result<u64, StreamResult> {
        self.begin_with(None)
    }
    fn begin_with(self: &Arc<Self>, ticks: Option<u64>) -> Result<u64, StreamResult> {
        let mut s = self.state.lock().unwrap();
        if self.closing.load(Ordering::Acquire) {
            return Err(StreamResult::Unavailable);
        }
        if s.active.is_some() {
            return Err(StreamResult::Busy);
        }
        if s.end != End::Open {
            return Err(s.end.result());
        }
        let Some(id) = s.next else {
            drop(s);
            self.close();
            return Err(StreamResult::Unavailable);
        };
        let timer = if let Some(delay) = ticks.filter(|delay| *delay > 0) {
            let Some(shared) = self
                .metrics
                .upgrade()
                .and_then(|m| m.runtime.get().and_then(Weak::upgrade))
            else {
                return Err(StreamResult::Unavailable);
            };
            match shared
                .clock
                .arm_stream(self.owner, id, delay, Arc::downgrade(self))
            {
                Ok(timer) => Some(timer),
                Err(error) => {
                    drop(s);
                    panic!("stream timer failed: {error:?}");
                }
            }
        } else {
            None
        };
        s.next = id.checked_add(1);
        let bytes = vec![0; self.size];
        self.counters
            .live_bytes
            .fetch_add(bytes.capacity() as u64, Ordering::Relaxed);
        self.counters.buffers.fetch_add(1, Ordering::Relaxed);
        self.counters.started.fetch_add(1, Ordering::Relaxed);
        s.active = Some(Active {
            id,
            timer,
            zero: ticks == Some(0),
            signals: Arc::new(Signals::default()),
            phase: Phase::Pending {
                bytes: StreamBytes {
                    bytes,
                    counters: self.counters.clone(),
                },
                attempted: false,
            },
        });
        self.counters.last_id.store(id, Ordering::Relaxed);
        self.counters.active_id.store(id, Ordering::Release);
        drop(s);
        self.wake();
        Ok(id)
    }
    fn cancel_request(&self, id: u64) -> bool {
        let mut s = self.state.lock().unwrap();
        let Some(active) = s.active.as_mut().filter(|active| active.id == id) else {
            return false;
        };
        active.signals.cancelled.store(true, Ordering::Release);
        active.timer = None;
        drop(s);
        self.wake();
        true
    }
    fn claim(&self, id: u64) -> Option<Request> {
        let mut s = self.state.lock().unwrap();
        if self.closing.load(Ordering::Acquire) {
            return None;
        }
        let active = s.active.as_ref()?;
        if active.id != id
            || active.signals.cancelled.load(Ordering::Acquire)
            || !matches!(active.phase, Phase::Pending { .. })
        {
            return None;
        }
        let endpoint = s.endpoint.take().expect("idle endpoint owner");
        let active = s.active.as_mut().unwrap();
        let Phase::Pending { bytes, .. } = std::mem::replace(&mut active.phase, Phase::Running)
        else {
            unreachable!()
        };
        Some(Request {
            id,
            signals: active.signals.clone(),
            endpoint,
            bytes,
        })
    }
    /// Only the matching running request may publish or return its endpoint.
    fn finish(&self, id: u64, outcome: Outcome) {
        let mut s = self.state.lock().unwrap();
        let Some(active) = s
            .active
            .as_ref()
            .filter(|a| a.id == id && matches!(a.phase, Phase::Running))
        else {
            return;
        };
        let closing = self.closing.load(Ordering::Acquire);
        let cancelled = closing || active.signals.cancelled.load(Ordering::Acquire);
        let expired = active.signals.expired.load(Ordering::Acquire);
        let zero = active.zero;
        let (endpoint, phase, end) = match outcome {
            Outcome::Pending(request) if !cancelled && !expired && !zero => {
                assert_eq!(request.id, id);
                (
                    Some(request.endpoint),
                    Some(Phase::Pending {
                        bytes: request.bytes,
                        attempted: true,
                    }),
                    End::Open,
                )
            }
            Outcome::Data(endpoint, bytes) if !cancelled => (
                Some(endpoint),
                Some(Phase::Ready(StreamResult::Data(bytes))),
                End::Open,
            ),
            Outcome::Eof if !cancelled => (None, Some(Phase::Ready(StreamResult::Eof)), End::Eof),
            Outcome::Failed if !cancelled => {
                (None, Some(Phase::Ready(StreamResult::Failed)), End::Failed)
            }
            Outcome::Pending(request) if !cancelled => {
                (Some(request.endpoint), Some(Phase::TimedOut), End::Open)
            }
            Outcome::Cancelled(endpoint) if !cancelled && expired => {
                (Some(endpoint), Some(Phase::TimedOut), End::Open)
            }
            Outcome::Pending(request) => (Some(request.endpoint), None, End::Open),
            Outcome::Data(endpoint, _) | Outcome::Cancelled(endpoint) => {
                (Some(endpoint), None, End::Open)
            }
            Outcome::Eof => (None, None, End::Eof),
            Outcome::Failed => (None, None, End::Failed),
        };
        s.endpoint = if closing {
            drop(endpoint);
            None
        } else {
            endpoint
        };
        s.end = if closing { End::Closed } else { end };
        let pending = matches!(phase, Some(Phase::Pending { .. }));
        let delivered_later = phase.is_some();
        if !pending {
            s.active.as_mut().unwrap().timer = None;
        }
        if matches!(phase, Some(Phase::TimedOut)) {
            self.counters.timeouts.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(phase) = phase {
            s.active.as_mut().unwrap().phase = phase;
        } else {
            s.active = None;
            self.counters.active_id.store(0, Ordering::Release);
        }
        if !pending {
            self.counters.settled.fetch_add(1, Ordering::Release);
            if !delivered_later {
                self.counters.discarded.fetch_add(1, Ordering::Release);
            }
        }
        drop(s);
        if !pending {
            self.notify();
        }
    }
    pub(super) fn expire(&self, key: timer::Key) -> bool {
        let mut s = self.state.lock().unwrap();
        if self.closing.load(Ordering::Acquire) || key.process != self.owner {
            return false;
        }
        let Some(active) = s.active.as_mut().filter(|a| {
            a.id == key.generation
                && !a.signals.cancelled.load(Ordering::Acquire)
                && a.timer.as_ref().is_some_and(|timer| timer.key == key)
        }) else {
            return false;
        };
        active.signals.expired.store(true, Ordering::Release);
        active.timer = None;
        let ready = match active.phase {
            Phase::Pending { .. } => {
                active.phase = Phase::TimedOut;
                self.counters.timeouts.fetch_add(1, Ordering::Relaxed);
                self.counters.settled.fetch_add(1, Ordering::Release);
                true
            }
            Phase::Running => false,
            Phase::Ready(_) | Phase::TimedOut => return false,
        };
        drop(s);
        self.wake();
        if ready {
            self.notify();
        }
        true
    }
    fn dispatch(&self, id: u64) -> bool {
        if let Some(request) = self.claim(id) {
            // Observe cancellation after claim, never a cached pre-claim value.
            let cancelled = self.closing.load(Ordering::Acquire)
                || request.signals.cancelled.load(Ordering::Acquire)
                || request.signals.expired.load(Ordering::Acquire);
            self.finish(id, perform(request, cancelled));
            true
        } else {
            false
        }
    }
    /// The driver alone retires endpoints, including idle endpoints.
    fn control(&self) {
        let mut s = self.state.lock().unwrap();
        let closing = self.closing.load(Ordering::Acquire);
        let cancel = s
            .active
            .as_ref()
            .is_some_and(|a| a.signals.cancelled.load(Ordering::Acquire));
        let running = s
            .active
            .as_ref()
            .is_some_and(|a| matches!(a.phase, Phase::Running));
        let mut notify = false;
        if (closing || cancel) && !running {
            if let Some(active) = s.active.take() {
                if matches!(active.phase, Phase::Pending { .. }) {
                    self.counters.settled.fetch_add(1, Ordering::Relaxed);
                }
                drop(active);
                self.counters.discarded.fetch_add(1, Ordering::Release);
                self.counters.active_id.store(0, Ordering::Release);
                notify = true;
            }
            if closing {
                s.endpoint = None;
                s.end = End::Closed;
            }
        }
        let initial = s.active.as_ref().and_then(|a| {
            matches!(
                a.phase,
                Phase::Pending {
                    attempted: false,
                    ..
                }
            )
            .then_some(a.id)
        });
        drop(s);
        if notify {
            self.notify();
        }
        if let Some(id) = initial {
            self.dispatch(id);
        }
    }
    fn quiescent_closed(&self) -> bool {
        let s = self.state.lock().unwrap();
        s.end != End::Open && s.endpoint.is_none() && s.active.is_none()
    }
    fn ticket(self: &Arc<Self>) -> Option<Ticket> {
        let s = self.state.lock().unwrap();
        let active = s.active.as_ref()?;
        if !matches!(active.phase, Phase::Pending { .. }) {
            return None;
        }
        Some(Ticket {
            stream: self.clone(),
            id: active.id,
            fd: s.endpoint.as_ref()?.socket.as_raw_fd(),
        })
    }
}
/// Owns the record through the poll call; only this driver changes fd ownership.
struct Ticket {
    stream: Arc<Stream>,
    id: u64,
    fd: std::os::fd::RawFd,
}
impl Ticket {
    fn dispatch(&self) -> bool {
        self.stream.dispatch(self.id)
    }
}
struct Table {
    closed: bool,
    slots: Vec<Option<Arc<Stream>>>,
}
#[cfg(test)]
struct PollGate {
    snapshot: mpsc::Sender<()>,
    resume: mpsc::Receiver<()>,
    result: mpsc::Sender<Vec<i16>>,
}
pub(super) struct Hub {
    table: Mutex<Table>,
    signal: Option<Mutex<Option<UnixStream>>>,
    stopped: AtomicBool,
    #[cfg(test)]
    poll_gate: Mutex<Option<PollGate>>,
}
impl Default for Hub {
    fn default() -> Self {
        Self {
            table: Mutex::new(Table {
                closed: true,
                slots: Vec::new(),
            }),
            signal: None,
            stopped: AtomicBool::new(true),
            #[cfg(test)]
            poll_gate: Mutex::new(None),
        }
    }
}
impl Hub {
    pub(super) fn new(capacity: usize) -> (Arc<Self>, UnixStream) {
        assert!((1..=LIMIT).contains(&capacity));
        let (reader, writer) = UnixStream::pair().expect("stream wake pair");
        reader.set_nonblocking(true).unwrap();
        writer.set_nonblocking(true).unwrap();
        (
            Arc::new(Self {
                table: Mutex::new(Table {
                    closed: false,
                    slots: (0..capacity).map(|_| None).collect(),
                }),
                signal: Some(Mutex::new(Some(writer))),
                stopped: AtomicBool::new(false),
                #[cfg(test)]
                poll_gate: Mutex::new(None),
            }),
            reader,
        )
    }
    fn wake(&self) {
        if let Some(signal) = &self.signal {
            let mut writer = signal.lock().unwrap();
            if let Some(endpoint) = writer.as_mut() {
                match endpoint.write(&[1]) {
                    Ok(_) => {}
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(_) => {
                        writer.take();
                    }
                }
            }
        }
    }
    pub(super) fn install(
        self: &Arc<Self>,
        metrics: &Arc<Metrics>,
        socket: UnixStream,
        size: usize,
    ) -> Result<StreamReceipt, ReadAdmissionError> {
        if !(1..=4096).contains(&size) {
            return Err(ReadAdmissionError::Size);
        }
        socket
            .set_nonblocking(true)
            .map_err(|_| ReadAdmissionError::Setup)?;
        let mut installed = metrics.stream.lock().unwrap();
        if installed.is_some() {
            return Err(ReadAdmissionError::AlreadyInstalled);
        }
        let mut table = self.table.lock().unwrap();
        if table.closed {
            return Err(ReadAdmissionError::Closed);
        }
        let slot = table
            .slots
            .iter_mut()
            .find(|s| s.is_none())
            .ok_or(ReadAdmissionError::Capacity)?;
        let counters = Arc::new(Counters::default());
        let stream = Arc::new(Stream {
            owner: *metrics.id.get().unwrap(),
            metrics: Arc::downgrade(metrics),
            hub: Arc::downgrade(self),
            size,
            closing: AtomicBool::new(false),
            counters: counters.clone(),
            state: Mutex::new(State {
                endpoint: Some(Endpoint {
                    socket,
                    counters: counters.clone(),
                }),
                next: Some(1),
                active: None,
                end: End::Open,
            }),
        });
        *slot = Some(stream.clone());
        *installed = Some(stream);
        drop(table);
        drop(installed);
        self.wake();
        Ok(StreamReceipt { counters })
    }
    pub(super) fn close(&self) {
        let mut table = self.table.lock().unwrap();
        table.closed = true;
        for s in table.slots.iter().flatten() {
            s.closing.store(true, Ordering::Release);
        }
        drop(table);
        self.wake();
    }
    pub(super) fn active(&self) -> usize {
        self.table.lock().unwrap().slots.iter().flatten().count()
    }
    pub(super) fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }
    fn streams(&self) -> Vec<Arc<Stream>> {
        self.table
            .lock()
            .unwrap()
            .slots
            .iter()
            .flatten()
            .cloned()
            .collect()
    }
    fn service(&self) {
        for stream in self.streams() {
            stream.control();
            if stream.quiescent_closed() {
                let mut table = self.table.lock().unwrap();
                for slot in &mut table.slots {
                    if slot
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, &stream))
                    {
                        *slot = None;
                        stream.counters.retired.store(true, Ordering::Release);
                        break;
                    }
                }
            }
        }
    }
    pub(super) fn run(self: Arc<Self>, mut control: UnixStream) {
        loop {
            let mut drain = [0_u8; 64];
            match control.read(&mut drain) {
                Ok(0) => self.close(),
                Err(e)
                    if !matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    self.close()
                }
                _ => {}
            }
            self.service();
            {
                let table = self.table.lock().unwrap();
                if table.closed && table.slots.iter().all(Option::is_none) {
                    break;
                }
            }
            let tickets: Vec<_> = self.streams().iter().filter_map(|s| s.ticket()).collect();
            let mut fds = Vec::with_capacity(LIMIT + 1);
            fds.push(libc::pollfd {
                fd: control.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            });
            for ticket in &tickets {
                fds.push(libc::pollfd {
                    fd: ticket.fd,
                    events: libc::POLLIN,
                    revents: 0,
                });
            }
            #[cfg(test)]
            let poll_gate = self.poll_gate.lock().unwrap().take();
            #[cfg(test)]
            if let Some(gate) = &poll_gate {
                gate.snapshot.send(()).unwrap();
                gate.resume.recv().unwrap();
            }
            if io_read::poll_ready(&mut fds).is_err() {
                self.close();
            }
            #[cfg(test)]
            if let Some(gate) = poll_gate {
                gate.result
                    .send(fds.iter().map(|fd| fd.revents).collect())
                    .unwrap();
            }
            match control.read(&mut drain) {
                Ok(0) => self.close(),
                Err(e)
                    if !matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    self.close()
                }
                _ => {}
            }
            // Wake/control changes are acted on before any readiness dispatch.
            // A concurrent actor stop is also checked by claim and perform.
            for (ticket, event) in tickets.iter().zip(&fds[1..]) {
                if event.revents != 0 {
                    ticket.dispatch();
                }
            }
        }
        self.stopped.store(true, Ordering::Release);
    }
}
pub(super) fn cancel(metrics: &Metrics) {
    if let Some(stream) = metrics.stream.lock().unwrap().as_ref() {
        stream.close();
    }
}
pub struct ReadNext {
    metrics: Arc<Metrics>,
    budget: Arc<Budget>,
    stream: Option<Arc<Stream>>,
    id: Option<u64>,
    immediate: Option<StreamResult>,
    finished: bool,
}
impl ProcessContext {
    pub fn read_next(&self) -> ReadNext {
        self.read_next_inner(None)
    }
    pub fn read_next_for(&self, ticks: u64) -> ReadNextFor {
        assert!(
            ticks <= u64::from(u32::MAX),
            "stream timeout exceeds u32 range"
        );
        ReadNextFor {
            inner: self.read_next_inner(Some(ticks)),
        }
    }
    fn read_next_inner(&self, ticks: Option<u64>) -> ReadNext {
        let stream = self.metrics.stream.lock().unwrap().clone();
        let begun = stream
            .as_ref()
            .map_or(Err(StreamResult::Unavailable), |s| match ticks {
                None => s.begin(),
                Some(_) => s.begin_with(ticks),
            });
        let (id, immediate) = match begun {
            Ok(id) => (Some(id), None),
            Err(result) => (None, Some(result)),
        };
        ReadNext {
            metrics: self.metrics.clone(),
            budget: self.budget.clone(),
            stream,
            id,
            immediate,
            finished: false,
        }
    }
}
impl ReadNext {
    fn poll_result(&mut self) -> Poll<Result<StreamResult, ()>> {
        let this = self;
        if !this.budget.charge() {
            return Poll::Pending;
        }
        if let Some(result) = this.immediate.take() {
            this.finished = true;
            return Poll::Ready(Ok(result));
        }
        let (Some(stream), Some(id)) = (&this.stream, this.id) else {
            return Poll::Ready(Ok(StreamResult::Unavailable));
        };
        let mut inbox = this.metrics.mailbox.lock().unwrap();
        let mut s = stream.state.lock().unwrap();
        let matching = s.active.as_ref().is_some_and(|a| a.id == id);
        if !matching {
            inbox.waiting = false;
            this.finished = true;
            return Poll::Ready(Ok(StreamResult::Unavailable));
        }
        if matches!(
            s.active.as_ref().unwrap().phase,
            Phase::Ready(_) | Phase::TimedOut
        ) {
            let active = s.active.take().unwrap();
            let result = match active.phase {
                Phase::Ready(value) => Ok(value),
                Phase::TimedOut => Err(()),
                _ => unreachable!(),
            };
            stream.counters.active_id.store(0, Ordering::Relaxed);
            stream.counters.delivered.fetch_add(1, Ordering::Release);
            inbox.waiting = false;
            this.finished = true;
            drop(s);
            drop(inbox);
            stream.wake();
            Poll::Ready(result)
        } else {
            inbox.waiting = true;
            Poll::Pending
        }
    }
}
impl Future for ReadNext {
    type Output = StreamResult;
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<StreamResult> {
        self.poll_result()
            .map(|result| result.unwrap_or_else(|_| unreachable!("untimed read expired")))
    }
}
pub struct ReadNextFor {
    inner: ReadNext,
}
impl Future for ReadNextFor {
    type Output = TimedStreamResult;
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<TimedStreamResult> {
        self.inner.poll_result().map(|result| match result {
            Ok(StreamResult::Data(bytes)) => TimedStreamResult::Data(bytes),
            Ok(StreamResult::Eof) => TimedStreamResult::Eof,
            Ok(StreamResult::Failed) => TimedStreamResult::Failed,
            Ok(StreamResult::Unavailable) => TimedStreamResult::Unavailable,
            Ok(StreamResult::Busy) => TimedStreamResult::Busy,
            Err(()) => TimedStreamResult::Timeout,
        })
    }
}
impl Drop for ReadNext {
    fn drop(&mut self) {
        if !self.finished {
            if let (Some(stream), Some(id)) = (&self.stream, self.id) {
                stream.cancel_request(id);
            }
        }
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod timed_tests;
