//! Correctness-only single-worker runtime for the checked mailbox profile.
#![deny(unsafe_code)]

use std::collections::{HashMap, LinkedList, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock, Weak};
use std::task::{Context, Poll, Wake, Waker};

mod lifecycle;
mod links;
mod mailbox;
mod monitor;
mod shared;
pub use links::{Exit, LinkError, LinkResult};
pub use mailbox::ExitResult;
pub use mailbox::{SharedReceive, SharedReceiveResult, TimedSharedReceive};
pub use shared::{SharedBytes, SharedEnvelope, SharedReceipt, SharedSnapshot};
mod timer;
pub use lifecycle::{
    KillReceipt, LookupError, ProcessId, Runtime, RuntimeClient, SpawnError, SpawnErrorKind,
};
pub use mailbox::{DownResult, Event, EventResult};
pub use mailbox::{Envelope, MessageReceipt, Receive, ReceiveResult, TimedReceive};
pub use monitor::{Down, MonitorError, MonitorRef};
pub use timer::{ClockError, ClockSnapshot};

/// Result of atomic monitored spawn. Copying this receipt retains no execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spawned {
    pid: ProcessId,
    reference: MonitorRef,
}
impl Spawned {
    pub fn pid(self) -> ProcessId {
        self.pid
    }
    pub fn reference(self) -> MonitorRef {
        self.reference
    }
}

type Frame = Pin<Box<dyn Future<Output = ()> + Send>>;
static NEXT_EVENT: AtomicU64 = AtomicU64::new(1);
fn event() -> u64 {
    NEXT_EVENT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .expect("logical event counter exhausted")
}

/// Opaque admitted program; unrestricted futures cannot enter safe spawning.
pub struct Program {
    make: fn(ProcessContext) -> Frame,
}
impl Program {
    /// # Safety
    /// All reachable application execution, types and destruction must satisfy
    /// this experiment's checked grammar and mandatory checkpoints. The factory
    /// only allocates a generated noncapturing frame. No unqualified callbacks,
    /// native futures, globals or destructors may enter. This is not a sandbox.
    #[allow(unsafe_code)]
    pub unsafe fn __from_generated(make: fn(ProcessContext) -> Frame) -> Self {
        Self { make }
    }
}

#[derive(Default)]
struct Metrics {
    lifecycle: Mutex<()>,
    mailbox: Mutex<mailbox::Mailbox>,
    id: OnceLock<ProcessId>,
    runtime: OnceLock<Weak<lifecycle::Shared>>,
    polls: AtomicU64,
    steps: AtomicU64,
    progress: AtomicU64,
    stage: AtomicU64,
    input_taken: AtomicBool,
    result: AtomicU64,
    allocation: Arc<Allocation>,
    polling: AtomicBool,
    kill_event: AtomicU64,
    stop_event: AtomicU64,
    cleanup_event: AtomicU64,
    terminal: AtomicU8,
    exit_reason: AtomicU64,
    pending_exit: AtomicU64,
    trap_exit: AtomicBool,
    scans: AtomicU64,
    receives: AtomicU64,
    parks: AtomicU64,
    wakes: AtomicU64,
    cleanup_batches: AtomicU64,
    cleanup_messages: AtomicU64,
    timeouts: AtomicU64,
}
struct Budget {
    remaining: AtomicU32,
}
impl Budget {
    fn charge(&self) -> bool {
        let left = self.remaining.load(Ordering::Relaxed);
        if left == 0 {
            false
        } else {
            self.remaining.store(left - 1, Ordering::Relaxed);
            true
        }
    }
}

/// Concrete movable input/payload storage. Counters follow allocation origin,
/// including when a buffer outlives its producing process after transfer.
#[derive(Default)]
struct Allocation {
    live_bytes: AtomicU64,
    freed_bytes: AtomicU64,
    drops: AtomicU64,
}
pub struct OwnedWords {
    words: Vec<u64>,
    allocation: Arc<Allocation>,
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
        drop(std::mem::take(&mut self.words));
        self.allocation
            .live_bytes
            .fetch_sub(bytes, Ordering::Relaxed);
        self.allocation
            .freed_bytes
            .fetch_add(bytes, Ordering::Relaxed);
        self.allocation.drops.fetch_add(1, Ordering::Release);
    }
}

pub struct ProcessContext {
    input: Option<OwnedWords>,
    tag: u64,
    argument: u64,
    target: Option<ProcessId>,
    budget: Arc<Budget>,
    metrics: Arc<Metrics>,
}
impl ProcessContext {
    pub fn send_shared(&self, target: ProcessId, tag: u64, bytes: SharedBytes) {
        let envelope = SharedEnvelope::new(tag, bytes);
        if let Some(shared) = self.metrics.runtime.get().and_then(Weak::upgrade) {
            RuntimeClient { shared }.send_shared(target, envelope);
        }
    }
    pub fn receive_shared(&self, tag: Option<u64>) -> SharedReceive {
        SharedReceive::new(self.metrics.clone(), self.budget.clone(), tag)
    }
    pub fn receive_shared_for(&self, tag: Option<u64>, ticks: u64) -> TimedSharedReceive {
        TimedSharedReceive::new(self.metrics.clone(), self.budget.clone(), tag, ticks)
    }
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
    pub fn self_id(&self) -> ProcessId {
        *self.metrics.id.get().expect("published identity")
    }
    pub fn input_target(&self) -> ProcessId {
        self.target.unwrap_or_else(|| self.self_id())
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
    pub fn receive(&self, tag: Option<u64>) -> Receive {
        Receive::new(self.metrics.clone(), self.budget.clone(), tag)
    }
    pub fn receive_for(&self, tag: Option<u64>, ticks: u64) -> TimedReceive {
        TimedReceive::new(self.metrics.clone(), self.budget.clone(), tag, ticks)
    }
    pub fn try_link(&self, target: ProcessId) -> LinkResult {
        let shared = self
            .metrics
            .runtime
            .get()
            .and_then(Weak::upgrade)
            .expect("live runtime");
        match (RuntimeClient { shared }).link(self.self_id(), target) {
            Ok(()) => LinkResult::Linked,
            Err(LinkError::NoProcess) => LinkResult::NoProcess,
            Err(error) => panic!("link origin failed: {error:?}"),
        }
    }
    pub fn link(&self, target: ProcessId) {
        if matches!(self.try_link(target), LinkResult::NoProcess) {
            self.metrics.pending_exit.store(4, Ordering::Release);
            panic!("link failed: NoProcess");
        }
    }
    pub fn unlink(&self, target: ProcessId) {
        if let Some(shared) = self.metrics.runtime.get().and_then(Weak::upgrade) {
            shared.links.unlink(self.self_id(), target);
        }
    }
    pub fn trap_exit(&self, enabled: bool) -> bool {
        let shared = self
            .metrics
            .runtime
            .get()
            .and_then(Weak::upgrade)
            .expect("live runtime");
        shared.links.set_trap(&self.metrics, enabled)
    }
    pub fn receive_exit(&self, from: ProcessId) -> mailbox::ExitReceive {
        mailbox::ExitReceive::new(self.metrics.clone(), self.budget.clone(), from)
    }
    pub fn receive_exit_for(&self, from: ProcessId, ticks: u64) -> mailbox::TimedExitReceive {
        mailbox::TimedExitReceive::new(self.metrics.clone(), self.budget.clone(), from, ticks)
    }
    pub fn monitor(&self, target: ProcessId) -> MonitorRef {
        let shared = self
            .metrics
            .runtime
            .get()
            .and_then(Weak::upgrade)
            .expect("live runtime");
        RuntimeClient { shared }
            .monitor(self.self_id(), target)
            .expect("monitor creation failed")
    }
    pub fn demonitor(&self, reference: MonitorRef) {
        if let Some(shared) = self.metrics.runtime.get().and_then(Weak::upgrade) {
            shared.monitors.cancel(self.self_id(), reference);
        }
    }
    pub fn demonitor_flush(&self, reference: MonitorRef) -> mailbox::Flush {
        self.demonitor(reference);
        mailbox::Flush::new(self.metrics.clone(), self.budget.clone(), reference)
    }
    pub fn receive_event(&self) -> mailbox::EventReceive {
        mailbox::EventReceive::new(self.metrics.clone(), self.budget.clone())
    }
    pub fn receive_event_for(&self, ticks: u64) -> mailbox::TimedEventReceive {
        mailbox::TimedEventReceive::new(self.metrics.clone(), self.budget.clone(), ticks)
    }
    pub fn receive_down(&self, reference: MonitorRef) -> mailbox::DownReceive {
        mailbox::DownReceive::new(self.metrics.clone(), self.budget.clone(), reference)
    }
    pub fn receive_down_for(&self, reference: MonitorRef, ticks: u64) -> mailbox::TimedDownReceive {
        mailbox::TimedDownReceive::new(self.metrics.clone(), self.budget.clone(), reference, ticks)
    }
    pub fn send(&self, target: ProcessId, tag: u64, words: OwnedWords) {
        let envelope = Envelope::from_owned(tag, words);
        if let Some(shared) = self.metrics.runtime.get().and_then(Weak::upgrade) {
            RuntimeClient { shared }.send(target, envelope);
        } // Otherwise the envelope drops here; no receiver owns it.
    }
}

pub struct Checkpoint<'a> {
    budget: &'a Budget,
}
impl Future for Checkpoint<'_> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        if self.budget.charge() {
            Poll::Ready(())
        } else {
            Poll::Pending
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
    pub result: u64,
    pub live_bytes: u64,
    pub freed_bytes: u64,
    pub drops: u64,
    pub kill_event: u64,
    pub stop_event: u64,
    pub cleanup_event: u64,
    /// 0 live, 1 completed, 2 killed, 3 failed, 4 rejected before publication.
    pub terminal: u8,
    /// Committed reason: 1 normal, 2 killed, 3 failure, 4 noproc (0 before stop).
    pub exit_reason: u64,
    pub pending_exit: u64,
    pub trap_exit: bool,
    pub polling: bool,
    pub waiting: bool,
    pub queued: usize,
    pub scanned_prefix: usize,
    pub scans: u64,
    pub receives: u64,
    pub parks: u64,
    pub wakes: u64,
    pub cleanup_batches: u64,
    pub cleanup_messages: u64,
    pub timeouts: u64,
    pub timer_generation: Option<u64>,
    pub timer_deadline: Option<u128>,
    pub timer_expired: bool,
}
#[derive(Clone)]
pub struct ProcessHandle {
    id: ProcessId,
    metrics: Arc<Metrics>,
}
impl ProcessHandle {
    pub fn id(&self) -> ProcessId {
        self.id
    }
    pub fn kill(&self) -> KillReceipt {
        lifecycle::request_kill(&self.metrics)
    }
    pub fn snapshot(&self) -> Snapshot {
        snapshot(&self.metrics)
    }
}
fn snapshot(m: &Metrics) -> Snapshot {
    let cleanup_event = m.cleanup_event.load(Ordering::Acquire);
    let terminal = m.terminal.load(Ordering::Acquire);
    let inbox = m.mailbox.lock().unwrap();
    Snapshot {
        polls: m.polls.load(Ordering::Relaxed),
        steps: m.steps.load(Ordering::Relaxed),
        progress: m.progress.load(Ordering::Relaxed),
        result: m.result.load(Ordering::Relaxed),
        stage: m.stage.load(Ordering::Acquire),
        input_taken: m.input_taken.load(Ordering::Acquire),
        live_bytes: m.allocation.live_bytes.load(Ordering::Relaxed),
        freed_bytes: m.allocation.freed_bytes.load(Ordering::Relaxed),
        drops: m.allocation.drops.load(Ordering::Relaxed),
        kill_event: m.kill_event.load(Ordering::Acquire),
        stop_event: m.stop_event.load(Ordering::Acquire),
        cleanup_event,
        terminal,
        exit_reason: m.exit_reason.load(Ordering::Acquire),
        pending_exit: m.pending_exit.load(Ordering::Acquire),
        trap_exit: m.trap_exit.load(Ordering::Acquire),
        polling: m.polling.load(Ordering::Acquire),
        waiting: inbox.waiting,
        queued: inbox.incoming.len() + inbox.saved.len(),
        scanned_prefix: inbox.saved.len(),
        scans: m.scans.load(Ordering::Relaxed),
        receives: m.receives.load(Ordering::Relaxed),
        parks: m.parks.load(Ordering::Relaxed),
        wakes: m.wakes.load(Ordering::Relaxed),
        cleanup_batches: m.cleanup_batches.load(Ordering::Relaxed),
        cleanup_messages: m.cleanup_messages.load(Ordering::Relaxed),
        timeouts: m.timeouts.load(Ordering::Relaxed),
        timer_generation: inbox.timer.as_ref().map(|timer| timer.key.generation),
        timer_deadline: inbox.timer.as_ref().map(|timer| timer.key.deadline),
        timer_expired: inbox.timer.as_ref().is_some_and(|timer| timer.expired),
    }
}
struct Task {
    id: ProcessId,
    frame: Frame,
    metrics: Arc<Metrics>,
    budget: Arc<Budget>,
}
enum Command {
    Spawn(Task),
    Wake(ProcessId),
    ClockAdvanced,
    MonitorWork,
    LinkWork,
    Start,
    Shutdown,
}
struct NoopWake;
impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}
/// Concrete host input; input_target defaults to this process unless spawn_to is used.
pub struct Input {
    pub words: Vec<u64>,
    pub tag: u64,
    pub argument: u64,
}

fn pending(task: Task, ready: &mut VecDeque<Task>, parked: &mut HashMap<ProcessId, Task>) {
    let inbox = task.metrics.mailbox.lock().unwrap();
    let should_park = inbox.waiting
        && task.metrics.kill_event.load(Ordering::Acquire) == 0
        && task.metrics.pending_exit.load(Ordering::Acquire) == 0;
    drop(inbox);
    if should_park {
        task.metrics.parks.fetch_add(1, Ordering::Relaxed);
        assert!(parked.insert(task.id, task).is_none());
    } else {
        ready.push_back(task);
    }
}
fn activate(id: ProcessId, ready: &mut VecDeque<Task>, parked: &mut HashMap<ProcessId, Task>) {
    if let Some(task) = parked.remove(&id) {
        task.metrics.mailbox.lock().unwrap().waiting = false;
        task.metrics.wakes.fetch_add(1, Ordering::Relaxed);
        ready.push_back(task);
    }
}
fn worker(
    receiver: mpsc::Receiver<Command>,
    allowance: u32,
    shared: Arc<lifecycle::Shared>,
    mut started: bool,
) {
    let mut ready = VecDeque::new();
    let mut parked = HashMap::new();
    let mut cleanup = VecDeque::new();
    let waker = Waker::from(Arc::new(NoopWake));
    let mut cx = Context::from_waker(&waker);
    let mut shutdown = false;
    loop {
        if !shutdown {
            for _ in 0..32 {
                let command = if (ready.is_empty()
                    && cleanup.is_empty()
                    && !shared.clock.has_due()
                    && !shared.monitors.has_pending()
                    && !shared.links.has_pending())
                    || !started
                {
                    receiver.recv().ok()
                } else {
                    match receiver.try_recv() {
                        Ok(c) => Some(c),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => None,
                    }
                };
                match command {
                    Some(Command::Spawn(task)) => ready.push_back(task),
                    Some(Command::Wake(id)) => activate(id, &mut ready, &mut parked),
                    Some(Command::ClockAdvanced) => {}
                    Some(Command::MonitorWork) => shared.monitors.acknowledge_wake(),
                    Some(Command::LinkWork) => shared.links.acknowledge_wake(),
                    Some(Command::Start) => started = true,
                    Some(Command::Shutdown) | None => {
                        shutdown = true;
                        break;
                    }
                }
            }
        }
        if shutdown {
            for task in ready.drain(..) {
                cleanup.push_back(lifecycle::retire(task, 2, &shared));
            }
            for (_, task) in parked.drain() {
                cleanup.push_back(lifecycle::retire(task, 2, &shared));
            }
        } else if started {
            // Limit due-timer work per turn, including when no application is runnable.
            shared.clock.service(8);
            shared.monitors.service(8);
            shared.links.service(8);
            if let Some(mut task) = ready.pop_front() {
                if task.metrics.kill_event.load(Ordering::Acquire) != 0 {
                    cleanup.push_back(lifecycle::retire(task, 2, &shared));
                } else if task.metrics.pending_exit.load(Ordering::Acquire) != 0 {
                    cleanup.push_back(lifecycle::retire(task, 3, &shared));
                } else {
                    shared.clock.application_poll();
                    task.budget.remaining.store(allowance, Ordering::Relaxed);
                    task.metrics.polling.store(true, Ordering::Release);
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        task.frame.as_mut().poll(&mut cx)
                    }));
                    task.metrics.polling.store(false, Ordering::Release);
                    task.metrics.polls.fetch_add(1, Ordering::Relaxed);
                    task.metrics.steps.fetch_add(
                        (allowance - task.budget.remaining.load(Ordering::Relaxed)) as u64,
                        Ordering::Relaxed,
                    );
                    let killed = task.metrics.kill_event.load(Ordering::Acquire) != 0;
                    match outcome {
                        _ if killed => cleanup.push_back(lifecycle::retire(task, 2, &shared)),
                        _ if task.metrics.pending_exit.load(Ordering::Acquire) != 0 => {
                            cleanup.push_back(lifecycle::retire(task, 3, &shared))
                        }
                        Ok(Poll::Ready(())) => {
                            cleanup.push_back(lifecycle::retire(task, 1, &shared))
                        }
                        Err(_) => cleanup.push_back(lifecycle::retire(task, 3, &shared)),
                        Ok(Poll::Pending) => pending(task, &mut ready, &mut parked),
                    }
                }
            }
        }
        if shutdown {
            shared.monitors.service(8);
            shared.links.service(8);
        }
        if let Some(mut item) = cleanup.pop_front() {
            if !item.step() {
                cleanup.push_back(item);
            }
        }
        if shutdown
            && cleanup.is_empty()
            && !shared.monitors.has_pending()
            && !shared.links.has_pending()
        {
            break;
        }
    }
}
