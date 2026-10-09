//! One host-installed, runtime-owned nonblocking socket read per process.
use super::*;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;

const MAX_OPERATIONS: usize = 16;
const MAX_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadAdmissionError {
    Closed,
    NotLive,
    AlreadyInstalled,
    Capacity,
    Size,
    Setup,
}

#[derive(Default)]
struct Counters {
    attempts: AtomicU64,
    settled: AtomicBool,
    delivered: AtomicBool,
    cancelled: AtomicBool,
    retired: AtomicBool,
    live_bytes: AtomicU64,
    storage_drops: AtomicU64,
    endpoint_drops: AtomicU64,
}
/// Counters only; this receipt does not retain a socket, buffer or actor frame.
#[derive(Clone)]
pub struct ReadReceipt {
    counters: Arc<Counters>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadSnapshot {
    pub attempts: u64,
    pub settled: bool,
    pub delivered: bool,
    pub cancelled: bool,
    pub retired: bool,
    pub live_bytes: u64,
    pub storage_drops: u64,
    pub endpoint_drops: u64,
}
impl ReadReceipt {
    /// Independent fields. Exact balances require a settled ownership boundary.
    pub fn snapshot(&self) -> ReadSnapshot {
        let c = &self.counters;
        ReadSnapshot {
            retired: c.retired.load(Ordering::Acquire),
            settled: c.settled.load(Ordering::Acquire),
            delivered: c.delivered.load(Ordering::Acquire),
            cancelled: c.cancelled.load(Ordering::Acquire),
            attempts: c.attempts.load(Ordering::Acquire),
            storage_drops: c.storage_drops.load(Ordering::Acquire),
            endpoint_drops: c.endpoint_drops.load(Ordering::Acquire),
            live_bytes: c.live_bytes.load(Ordering::Acquire),
        }
    }
}
pub struct ReadBytes {
    bytes: Vec<u8>,
    counters: Arc<Counters>,
}
impl ReadBytes {
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
impl Drop for ReadBytes {
    fn drop(&mut self) {
        drop(std::mem::take(&mut self.bytes));
        self.counters.live_bytes.store(0, Ordering::Relaxed);
        self.counters.storage_drops.fetch_add(1, Ordering::Release);
    }
}
pub enum ReadResult {
    Data(ReadBytes),
    Eof,
    Failed,
    Unavailable,
}
struct Endpoint {
    stream: UnixStream,
    counters: Arc<Counters>,
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        self.counters.endpoint_drops.fetch_add(1, Ordering::Release);
    }
}
struct Request {
    endpoint: Endpoint,
    bytes: ReadBytes,
}
enum Phase {
    Pending(Request),
    Running,
    Ready(ReadResult),
    Retired,
}
pub(super) struct Operation {
    owner: ProcessId,
    metrics: Weak<Metrics>,
    hub: Weak<Hub>,
    counters: Arc<Counters>,
    state: Mutex<Phase>,
    taken: AtomicBool,
}
impl Operation {
    fn cancel(&self) {
        if !self.counters.delivered.load(Ordering::Acquire)
            && !self.counters.retired.load(Ordering::Acquire)
        {
            self.counters.cancelled.store(true, Ordering::Release);
        }
        if let Some(hub) = self.hub.upgrade() {
            hub.wake();
        }
    }
    /// Taking the request leaves no borrow into actor or operation storage.
    fn claim(&self) -> Option<Request> {
        let mut state = self.state.lock().unwrap();
        if !matches!(*state, Phase::Pending(_)) {
            return None;
        }
        match std::mem::replace(&mut *state, Phase::Running) {
            Phase::Pending(request) => Some(request),
            _ => unreachable!(),
        }
    }
    fn finish(&self, outcome: Attempt) {
        let mut state = self.state.lock().unwrap();
        // A stale or duplicate completion owns and drops only its own result.
        if !matches!(*state, Phase::Running) {
            return;
        }
        let cancelled = self.counters.cancelled.load(Ordering::Acquire);
        match outcome {
            Attempt::Pending(request) if !cancelled => {
                *state = Phase::Pending(request);
                return;
            }
            Attempt::Done(result) if !cancelled => *state = Phase::Ready(result),
            _ => *state = Phase::Retired,
        }
        self.counters.settled.store(true, Ordering::Release);
        drop(state);
        if let Some(metrics) = self.metrics.upgrade() {
            if metrics.id.get() == Some(&self.owner) {
                mailbox::notify(&metrics);
            }
        }
    }
}
enum Attempt {
    Pending(Request),
    Done(ReadResult),
}
fn attempt(mut request: Request, cancelled: bool) -> Attempt {
    if cancelled {
        return Attempt::Done(ReadResult::Unavailable);
    }
    request
        .bytes
        .counters
        .attempts
        .fetch_add(1, Ordering::Relaxed);
    match request.endpoint.stream.read(&mut request.bytes.bytes) {
        Ok(0) => Attempt::Done(ReadResult::Eof),
        Ok(n) => {
            request.bytes.bytes.truncate(n);
            Attempt::Done(ReadResult::Data(request.bytes))
        }
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Attempt::Pending(request)
        }
        Err(_) => Attempt::Done(ReadResult::Failed),
    }
}
struct Table {
    closed: bool,
    slots: Vec<Option<Arc<Operation>>>,
}
pub(super) struct Hub {
    table: Mutex<Table>,
    signal: Option<Mutex<Option<UnixStream>>>,
    stopped: AtomicBool,
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
        }
    }
}
impl Hub {
    pub(super) fn new(capacity: usize) -> (Arc<Self>, UnixStream) {
        assert!((1..=MAX_OPERATIONS).contains(&capacity));
        let (reader, writer) = UnixStream::pair().expect("read driver wake pair");
        reader
            .set_nonblocking(true)
            .expect("nonblocking wake reader");
        writer
            .set_nonblocking(true)
            .expect("nonblocking wake writer");
        (
            Arc::new(Self {
                table: Mutex::new(Table {
                    closed: false,
                    slots: (0..capacity).map(|_| None).collect(),
                }),
                signal: Some(Mutex::new(Some(writer))),
                stopped: AtomicBool::new(false),
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
                    // Do not retry an interrupted write under actor lifecycle
                    // locks. Closing the sole writer reliably wakes poll with
                    // EOF/HUP; the driver fails closed and settles all reads.
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
        stream: UnixStream,
        size: usize,
    ) -> Result<ReadReceipt, ReadAdmissionError> {
        if !(1..=MAX_BYTES).contains(&size) {
            return Err(ReadAdmissionError::Size);
        }
        stream
            .set_nonblocking(true)
            .map_err(|_| ReadAdmissionError::Setup)?;
        let mut installed = metrics.read_operation.lock().unwrap();
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
        let bytes = vec![0; size];
        counters
            .live_bytes
            .store(bytes.capacity() as u64, Ordering::Relaxed);
        let operation = Arc::new(Operation {
            owner: *metrics.id.get().expect("published read owner"),
            metrics: Arc::downgrade(metrics),
            hub: Arc::downgrade(self),
            counters: counters.clone(),
            taken: AtomicBool::new(false),
            state: Mutex::new(Phase::Pending(Request {
                endpoint: Endpoint {
                    stream,
                    counters: counters.clone(),
                },
                bytes: ReadBytes {
                    bytes,
                    counters: counters.clone(),
                },
            })),
        });
        *slot = Some(operation.clone());
        *installed = Some(operation);
        drop(table);
        drop(installed);
        self.wake();
        Ok(ReadReceipt { counters })
    }
    pub(super) fn close(&self) {
        let mut table = self.table.lock().unwrap();
        table.closed = true;
        for op in table.slots.iter().flatten() {
            if !op.counters.delivered.load(Ordering::Acquire) {
                op.counters.cancelled.store(true, Ordering::Release);
            }
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
    fn operations(&self) -> Vec<Arc<Operation>> {
        self.table
            .lock()
            .unwrap()
            .slots
            .iter()
            .flatten()
            .cloned()
            .collect()
    }
    /// At most sixteen single nonblocking reads; no syscall holds these locks.
    fn service(&self) {
        for op in self.operations() {
            if let Some(request) = op.claim() {
                let cancelled = op.counters.cancelled.load(Ordering::Acquire);
                op.finish(attempt(request, cancelled));
            }
            if op.counters.cancelled.load(Ordering::Acquire) {
                let mut state = op.state.lock().unwrap();
                if !matches!(*state, Phase::Running) {
                    *state = Phase::Retired;
                    op.counters.settled.store(true, Ordering::Release);
                }
            }
            let mut table = self.table.lock().unwrap();
            if matches!(*op.state.lock().unwrap(), Phase::Retired) {
                for slot in &mut table.slots {
                    if slot
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, &op))
                    {
                        *slot = None;
                        op.counters.retired.store(true, Ordering::Release);
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
            let table = self.table.lock().unwrap();
            if table.closed && table.slots.iter().all(Option::is_none) {
                break;
            }
            let mut fds = Vec::with_capacity(MAX_OPERATIONS + 1);
            fds.push(libc::pollfd {
                fd: control.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            });
            for op in table.slots.iter().flatten() {
                if let Phase::Pending(request) = &*op.state.lock().unwrap() {
                    fds.push(libc::pollfd {
                        fd: request.endpoint.stream.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    });
                }
            }
            drop(table);
            if poll_ready(&mut fds).is_err() {
                self.close();
            }
        }
        self.stopped.store(true, Ordering::Release);
    }
}
#[allow(unsafe_code)]
pub(super) fn poll_ready(fds: &mut [libc::pollfd]) -> io::Result<()> {
    // SAFETY: the slice is writable for its declared length. This sole driver
    // retains all endpoint owners across poll, and poll retains no pointers.
    let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, -1) };
    if n < 0 {
        let e = io::Error::last_os_error();
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
    Ok(())
}

pub(super) fn cancel(metrics: &Metrics) {
    if let Some(op) = metrics.read_operation.lock().unwrap().as_ref() {
        op.cancel();
    }
}
pub struct ReadInput {
    metrics: Arc<Metrics>,
    budget: Arc<Budget>,
    op: Option<Arc<Operation>>,
}
impl ProcessContext {
    pub fn read_input(&self) -> ReadInput {
        let op = self
            .metrics
            .read_operation
            .lock()
            .unwrap()
            .clone()
            .filter(|op| !op.taken.swap(true, Ordering::AcqRel));
        ReadInput {
            metrics: self.metrics.clone(),
            budget: self.budget.clone(),
            op,
        }
    }
}
impl Future for ReadInput {
    type Output = ReadResult;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<ReadResult> {
        if !self.budget.charge() {
            return Poll::Pending;
        }
        let Some(op) = &self.op else {
            return Poll::Ready(ReadResult::Unavailable);
        };
        // Wait enrollment and notification use the mailbox lock. Publication
        // releases the operation lock before requesting the corresponding wake.
        let mut inbox = self.metrics.mailbox.lock().unwrap();
        let mut state = op.state.lock().unwrap();
        if matches!(*state, Phase::Ready(_)) {
            let Phase::Ready(result) = std::mem::replace(&mut *state, Phase::Retired) else {
                unreachable!()
            };
            inbox.waiting = false;
            op.counters.delivered.store(true, Ordering::Release);
            drop(state);
            drop(inbox);
            if let Some(hub) = op.hub.upgrade() {
                hub.wake();
            }
            Poll::Ready(result)
        } else if matches!(*state, Phase::Retired) {
            Poll::Ready(ReadResult::Unavailable)
        } else {
            inbox.waiting = true;
            Poll::Pending
        }
    }
}
#[cfg(test)]
mod tests;

impl Drop for ReadInput {
    fn drop(&mut self) {
        if let Some(op) = &self.op {
            if !op.counters.delivered.load(Ordering::Acquire) {
                op.cancel();
            }
        }
    }
}
