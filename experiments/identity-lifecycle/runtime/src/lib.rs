//! Research public boundary and one-worker executor for checked native programs.
#![deny(unsafe_code)]

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Instant;

mod lifecycle;
pub use lifecycle::{
    KillReceipt, LookupError, ProcessId, Runtime, RuntimeClient, SpawnError, SpawnErrorKind,
};

type Frame = Pin<Box<dyn Future<Output = ()> + Send>>;

/// A generated program. Safe callers cannot substitute an arbitrary future.
pub struct Program {
    make: fn(ProcessContext) -> Frame,
}

impl Program {
    /// Construct a program after checking all reachable execution and cleanup.
    ///
    /// # Safety
    /// The factory must create only a generated, noncapturing native frame whose
    /// every cycle and statement uses the mandatory budget checkpoint. All body
    /// operations and owned state must belong to this experiment's closed
    /// grammar. No unqualified callbacks, futures, globals or destructors may
    /// enter the frame. The factory itself must only allocate that frame.
    /// The matching macro reconstructs this grammar; ordinary consumers must
    /// not call this escape hatch. This is also a scheduling/resource contract.
    #[allow(unsafe_code)]
    pub unsafe fn __from_generated(make: fn(ProcessContext) -> Frame) -> Self {
        Self { make }
    }
}

#[derive(Default)]
struct Metrics {
    lifecycle: Mutex<()>,
    polls: AtomicU64,
    steps: AtomicU64,
    progress: AtomicU64,
    stage: AtomicU64,
    input_taken: AtomicBool,
    frame_bytes: AtomicU64,
    result: AtomicU64,
    live_bytes: AtomicU64,
    freed_bytes: AtomicU64,
    drops: AtomicU64,
    drops_during_poll: AtomicU64,
    polling: AtomicBool,
    kill_ns: AtomicU64,
    kill_during_poll: AtomicBool,
    stop_ns: AtomicU64,
    cleanup_ns: AtomicU64,
    first_poll_ns: AtomicU64,
    submitted_ns: AtomicU64,
    ready_delay_max_ns: AtomicU64,
    poll_max_ns: AtomicU64,
    terminal: AtomicU8,
}

struct Budget {
    remaining: AtomicU32,
}

/// Concrete trusted data: no downstream destructor, conversion or indexing.
pub struct OwnedWords {
    words: Vec<u64>,
    metrics: Arc<Metrics>,
}

impl OwnedWords {
    pub fn len(&self) -> u64 {
        self.words.len() as u64
    }
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
    pub fn read(&self, index: u64) -> u64 {
        self.words[index as usize]
    }
    pub fn write(&mut self, index: u64, value: u64) {
        self.words[index as usize] = value;
    }
}

impl Drop for OwnedWords {
    fn drop(&mut self) {
        let bytes = self.words.capacity() as u64 * 8;
        // Complete actual field destruction before reporting logical release.
        drop(std::mem::take(&mut self.words));
        self.metrics.live_bytes.fetch_sub(bytes, Ordering::Relaxed);
        self.metrics.freed_bytes.fetch_add(bytes, Ordering::Relaxed);
        self.metrics.drops.fetch_add(1, Ordering::Relaxed);
        if self.metrics.polling.load(Ordering::Relaxed) {
            self.metrics
                .drops_during_poll
                .fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub struct ProcessContext {
    input: Option<OwnedWords>,
    tag: u64,
    argument: u64,
    budget: Arc<Budget>,
    metrics: Arc<Metrics>,
}

impl ProcessContext {
    pub fn owned_input(&mut self) -> OwnedWords {
        let input = self.input.take().expect("input already taken");
        self.metrics.input_taken.store(true, Ordering::Release);
        input
    }
    pub fn input_tag(&self) -> u64 {
        self.tag
    }
    pub fn input_argument(&self) -> u64 {
        self.argument
    }
    pub fn mark(&self, value: u64) {
        self.metrics.stage.store(value, Ordering::Release);
    }
    pub fn observe(&self, value: u64) {
        self.metrics.progress.store(value, Ordering::Relaxed);
    }
    pub fn result(&self, value: u64) {
        self.metrics.result.store(value, Ordering::Relaxed);
    }
    pub fn checkpoint(&self) -> Checkpoint<'_> {
        Checkpoint {
            budget: &self.budget,
        }
    }
}

pub struct Checkpoint<'a> {
    budget: &'a Budget,
}
impl Future for Checkpoint<'_> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let left = self.budget.remaining.load(Ordering::Relaxed);
        if left == 0 {
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            self.budget.remaining.store(left - 1, Ordering::Relaxed);
            Poll::Ready(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub polls: u64,
    pub steps: u64,
    pub progress: u64,
    pub stage: u64,
    pub input_taken: bool,
    pub frame_bytes: u64,
    pub result: u64,
    pub live_bytes: u64,
    pub freed_bytes: u64,
    pub drops: u64,
    pub drops_during_poll: u64,
    pub kill_ns: u64,
    pub kill_during_poll: bool,
    pub stop_ns: u64,
    pub cleanup_ns: u64,
    pub first_poll_ns: u64,
    pub submitted_ns: u64,
    pub ready_delay_max_ns: u64,
    pub poll_max_ns: u64,
    /// 0 live, 1 completed, 2 killed, 3 failed, 4 rejected before publication.
    pub terminal: u8,
    pub polling: bool,
}

#[derive(Clone)]
pub struct ProcessHandle {
    id: ProcessId,
    metrics: Arc<Metrics>,
    epoch: Instant,
}

fn now(epoch: Instant) -> u64 {
    epoch.elapsed().as_nanos() as u64 + 1
}

impl ProcessHandle {
    pub fn id(&self) -> ProcessId {
        self.id
    }
    /// A receipt records whether kill won before terminal commitment.
    pub fn kill(&self) -> KillReceipt {
        lifecycle::request_kill(&self.metrics, self.epoch)
    }
    pub fn snapshot(&self) -> Snapshot {
        snapshot(&self.metrics)
    }
}

fn snapshot(m: &Metrics) -> Snapshot {
    // Completion is published after drop and observed before other fields.
    let cleanup_ns = m.cleanup_ns.load(Ordering::Acquire);
    let terminal = m.terminal.load(Ordering::Acquire);
    Snapshot {
        polls: m.polls.load(Ordering::Relaxed),
        steps: m.steps.load(Ordering::Relaxed),
        progress: m.progress.load(Ordering::Relaxed),
        result: m.result.load(Ordering::Relaxed),
        stage: m.stage.load(Ordering::Acquire),
        input_taken: m.input_taken.load(Ordering::Acquire),
        frame_bytes: m.frame_bytes.load(Ordering::Relaxed),
        live_bytes: m.live_bytes.load(Ordering::Relaxed),
        freed_bytes: m.freed_bytes.load(Ordering::Relaxed),
        drops: m.drops.load(Ordering::Relaxed),
        drops_during_poll: m.drops_during_poll.load(Ordering::Relaxed),
        kill_ns: m.kill_ns.load(Ordering::Acquire),
        kill_during_poll: m.kill_during_poll.load(Ordering::Acquire),
        stop_ns: m.stop_ns.load(Ordering::Acquire),
        cleanup_ns,
        first_poll_ns: m.first_poll_ns.load(Ordering::Relaxed),
        submitted_ns: m.submitted_ns.load(Ordering::Relaxed),
        ready_delay_max_ns: m.ready_delay_max_ns.load(Ordering::Relaxed),
        poll_max_ns: m.poll_max_ns.load(Ordering::Relaxed),
        terminal,
        polling: m.polling.load(Ordering::Acquire),
    }
}

struct Task {
    id: ProcessId,
    frame: Frame,
    metrics: Arc<Metrics>,
    budget: Arc<Budget>,
    ready_ns: u64,
}
enum Command {
    Spawn(Task),
    Start,
    Shutdown,
}
struct NoopWake;
impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

/// Concrete host input for this experiment; the checked program constructs its
/// private typed request from these scalar fields. This is not a mailbox ABI.
pub struct Input {
    pub words: Vec<u64>,
    pub tag: u64,
    pub argument: u64,
}

fn worker(
    receiver: mpsc::Receiver<Command>,
    allowance: u32,
    shared: Arc<lifecycle::Shared>,
    mut started: bool,
) {
    let epoch = shared.epoch;
    let mut queue = VecDeque::new();
    let waker = Waker::from(Arc::new(NoopWake));
    let mut cx = Context::from_waker(&waker);
    let mut shutdown = false;
    loop {
        // A bounded admission batch prevents a publishing host from monopolizing
        // this loop. This slice has no I/O waiters or application mailbox.
        for _ in 0..32 {
            let command = if queue.is_empty() || !started {
                receiver.recv().ok()
            } else {
                match receiver.try_recv() {
                    Ok(command) => Some(command),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => None,
                }
            };
            match command {
                Some(Command::Spawn(task)) => queue.push_back(task),
                Some(Command::Start) => {
                    started = true;
                }
                Some(Command::Shutdown) | None => {
                    shutdown = true;
                    break;
                }
            }
        }
        if shutdown {
            for task in queue.drain(..) {
                lifecycle::retire(task, 2, &shared);
            }
            break;
        }
        if !started {
            continue;
        }
        let Some(mut task) = queue.pop_front() else {
            continue;
        };
        if task.metrics.kill_ns.load(Ordering::Acquire) != 0 {
            lifecycle::retire(task, 2, &shared);
            continue;
        }
        let start = now(epoch);
        task.metrics
            .first_poll_ns
            .compare_exchange(0, start, Ordering::Relaxed, Ordering::Relaxed)
            .ok();
        task.metrics
            .ready_delay_max_ns
            .fetch_max(start - task.ready_ns, Ordering::Relaxed);
        task.budget.remaining.store(allowance, Ordering::Relaxed);
        task.metrics.polling.store(true, Ordering::Release);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            task.frame.as_mut().poll(&mut cx)
        }));
        task.metrics.polling.store(false, Ordering::Release);
        let end = now(epoch);
        task.metrics.polls.fetch_add(1, Ordering::Relaxed);
        task.metrics.steps.fetch_add(
            (allowance - task.budget.remaining.load(Ordering::Relaxed)) as u64,
            Ordering::Relaxed,
        );
        task.metrics
            .poll_max_ns
            .fetch_max(end - start, Ordering::Relaxed);
        let kill_ns = task.metrics.kill_ns.load(Ordering::Acquire);
        let killed = kill_ns != 0;
        if killed {
            task.metrics
                .kill_during_poll
                .store(kill_ns >= start && kill_ns <= end, Ordering::Release);
        }
        match outcome {
            _ if killed => lifecycle::retire(task, 2, &shared),
            Ok(Poll::Ready(())) => lifecycle::retire(task, 1, &shared),
            Err(_) => lifecycle::retire(task, 3, &shared),
            Ok(Poll::Pending) => {
                task.ready_ns = end;
                queue.push_back(task);
            }
        }
    }
}
