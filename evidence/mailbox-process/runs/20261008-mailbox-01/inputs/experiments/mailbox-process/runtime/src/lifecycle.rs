use super::*;
use std::sync::atomic::AtomicUsize;
use std::thread::JoinHandle;

static NEXT_RUNTIME: AtomicU64 = AtomicU64::new(1);

fn next_namespace(counter: &AtomicU64) -> Option<u64> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .ok()
}

/// Copyable local identity; never a pointer or a lease on application life.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProcessId {
    runtime: u64,
    slot: usize,
    generation: u64,
}
impl ProcessId {
    pub fn runtime(self) -> u64 {
        self.runtime
    }
    pub fn slot(self) -> usize {
        self.slot
    }
    pub fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KillReceipt {
    Accepted,
    AlreadyRequested,
    TooLate,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookupError {
    WrongRuntime,
    NotLive,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnErrorKind {
    InputCapacity,
    Capacity,
    Closed,
    Disconnected,
}
#[derive(Debug)]
pub struct SpawnError {
    pub kind: SpawnErrorKind,
    /// Rejected prepared frames are destroyed before return. Invalid input is
    /// rejected before tracked ownership and has no prepared-frame snapshot.
    pub cleanup: Option<Snapshot>,
}

struct Slot {
    generation: u64,
    record: Option<Arc<Metrics>>,
}
struct Registry {
    accepting: bool,
    slots: Vec<Slot>,
    free: Vec<usize>,
    live: usize,
    exhausted: usize,
}
impl Registry {
    fn new(capacity: usize) -> Self {
        Self {
            accepting: true,
            slots: (0..capacity)
                .map(|_| Slot {
                    generation: 1,
                    record: None,
                })
                .collect(),
            free: (0..capacity).rev().collect(),
            live: 0,
            exhausted: 0,
        }
    }
    fn remove(&mut self, id: ProcessId, metrics: &Arc<Metrics>) {
        let slot = &mut self.slots[id.slot];
        assert_eq!(slot.generation, id.generation);
        assert!(Arc::ptr_eq(
            slot.record.as_ref().expect("published record"),
            metrics
        ));
        slot.record = None;
        self.live -= 1;
        if let Some(generation) = slot.generation.checked_add(1) {
            slot.generation = generation;
            self.free.push(id.slot);
        } else {
            self.exhausted += 1; // Never reuse a retained complete identity.
        }
    }
}

pub(super) struct Shared {
    namespace: u64,
    registry: Mutex<Registry>,
    sender: mpsc::Sender<Command>,
    rejected: AtomicUsize,
}
impl Shared {
    pub(super) fn wake(&self, id: ProcessId) {
        let _ = self.sender.send(Command::Wake(id));
    }
}
struct Prepared {
    frame: Frame,
    metrics: Arc<Metrics>,
    budget: Arc<Budget>,
}
impl Prepared {
    fn build(
        program: Program,
        input: Input,
        shared: &Arc<Shared>,
        target: Option<ProcessId>,
    ) -> Result<Self, SpawnError> {
        if input.words.capacity() > 8192 {
            return Err(SpawnError {
                kind: SpawnErrorKind::InputCapacity,
                cleanup: None,
            });
        }
        let metrics = Arc::new(Metrics::default());
        metrics.runtime.set(Arc::downgrade(shared)).unwrap();
        metrics
            .allocation
            .live_bytes
            .store(input.words.capacity() as u64 * 8, Ordering::Relaxed);
        let budget = Arc::new(Budget {
            remaining: AtomicU32::new(0),
        });
        let context = ProcessContext {
            input: Some(OwnedWords {
                words: input.words,
                allocation: metrics.allocation.clone(),
            }),
            target,
            tag: input.tag,
            argument: input.argument,
            budget: budget.clone(),
            metrics: metrics.clone(),
        };
        let frame = (program.make)(context);
        Ok(Self {
            frame,
            metrics,
            budget,
        })
    }
    fn reject(self, kind: SpawnErrorKind, shared: &Shared) -> SpawnError {
        let metrics = self.metrics.clone();
        metrics.stop_event.store(event(), Ordering::Release);
        metrics.terminal.store(4, Ordering::Release);
        drop(self);
        metrics.cleanup_event.store(event(), Ordering::Release);
        shared.rejected.fetch_add(1, Ordering::Relaxed);
        SpawnError {
            kind,
            cleanup: Some(snapshot(&metrics)),
        }
    }
}

/// Cloneable host access survives Runtime shutdown only as a closed endpoint.
#[derive(Clone)]
pub struct RuntimeClient {
    pub(super) shared: Arc<Shared>,
}
impl RuntimeClient {
    pub fn spawn(&self, program: Program, words: Vec<u64>) -> Result<ProcessHandle, SpawnError> {
        self.spawn_input(
            program,
            Input {
                words,
                tag: 0,
                argument: 0,
            },
        )
    }
    pub fn spawn_input(&self, program: Program, input: Input) -> Result<ProcessHandle, SpawnError> {
        self.publish(Prepared::build(program, input, &self.shared, None)?)
    }
    pub fn spawn_to(
        &self,
        program: Program,
        input: Input,
        target: ProcessId,
    ) -> Result<ProcessHandle, SpawnError> {
        self.publish(Prepared::build(program, input, &self.shared, Some(target))?)
    }
    /// Ordinary send transfers ownership; absent/foreign/stale local IDs discard
    /// it silently. Return is not an acknowledgement of application receipt.
    pub fn send(&self, id: ProcessId, envelope: Envelope) {
        if let Ok(handle) = self.lookup(id) {
            mailbox::deliver(&handle.metrics, envelope);
        }
    }
    fn publish(&self, prepared: Prepared) -> Result<ProcessHandle, SpawnError> {
        let mut registry = self.shared.registry.lock().unwrap();
        let rejection = if !registry.accepting {
            Some(SpawnErrorKind::Closed)
        } else if registry.free.is_empty() {
            Some(SpawnErrorKind::Capacity)
        } else {
            None
        };
        if let Some(kind) = rejection {
            drop(registry);
            return Err(prepared.reject(kind, &self.shared));
        }
        let index = registry.free.pop().unwrap();
        let id = ProcessId {
            runtime: self.shared.namespace,
            slot: index,
            generation: registry.slots[index].generation,
        };
        let metrics = prepared.metrics.clone();
        metrics.id.set(id).unwrap();
        registry.slots[index].record = Some(metrics.clone());
        registry.live += 1;
        let task = Task {
            id,
            frame: prepared.frame,
            metrics: prepared.metrics,
            budget: prepared.budget,
        };
        // All publishers and shutdown serialize here. No lookup can see a
        // rejected identity, and accepted commands precede the shutdown marker.
        if let Err(mpsc::SendError(Command::Spawn(task))) =
            self.shared.sender.send(Command::Spawn(task))
        {
            registry.remove(id, &metrics);
            registry.accepting = false;
            drop(registry);
            let prepared = Prepared {
                frame: task.frame,
                metrics: task.metrics,
                budget: task.budget,
            };
            return Err(prepared.reject(SpawnErrorKind::Disconnected, &self.shared));
        }
        drop(registry);
        Ok(ProcessHandle { id, metrics })
    }
    pub fn lookup(&self, id: ProcessId) -> Result<ProcessHandle, LookupError> {
        if id.runtime != self.shared.namespace {
            return Err(LookupError::WrongRuntime);
        }
        let registry = self.shared.registry.lock().unwrap();
        let slot = registry.slots.get(id.slot).ok_or(LookupError::NotLive)?;
        if slot.generation != id.generation {
            return Err(LookupError::NotLive);
        }
        let metrics = slot.record.clone().ok_or(LookupError::NotLive)?;
        Ok(ProcessHandle { id, metrics })
    }
    pub fn kill(&self, id: ProcessId) -> Result<KillReceipt, LookupError> {
        Ok(self.lookup(id)?.kill())
    }
    /// Instantaneous observation only, never a lease or a precondition for send.
    /// The guard keeps the borrowed record valid without retaining an Arc.
    pub fn is_live(&self, id: ProcessId) -> bool {
        if id.runtime != self.shared.namespace {
            return false;
        }
        let registry = self.shared.registry.lock().unwrap();
        registry
            .slots
            .get(id.slot)
            .is_some_and(|slot| slot.generation == id.generation && slot.record.is_some())
    }
    pub fn live_count(&self) -> usize {
        self.shared.registry.lock().unwrap().live
    }
    pub fn exhausted_slots(&self) -> usize {
        self.shared.registry.lock().unwrap().exhausted
    }
    pub fn rejected_frames(&self) -> usize {
        self.shared.rejected.load(Ordering::Relaxed)
    }
    fn close(&self) {
        let mut registry = self.shared.registry.lock().unwrap();
        if registry.accepting {
            registry.accepting = false;
            let _ = self.shared.sender.send(Command::Shutdown);
        }
    }
}

pub struct Runtime {
    client: RuntimeClient,
    worker: Option<JoinHandle<()>>,
}
impl Runtime {
    pub fn new(allowance: u32) -> Self {
        Self::create(allowance, 1024, true)
    }
    pub fn paused(allowance: u32) -> Self {
        Self::create(allowance, 1024, false)
    }
    pub fn with_capacity(allowance: u32, capacity: usize) -> Self {
        Self::create(allowance, capacity, true)
    }
    pub fn paused_with_capacity(allowance: u32, capacity: usize) -> Self {
        Self::create(allowance, capacity, false)
    }
    fn create(allowance: u32, capacity: usize, started: bool) -> Self {
        assert!(allowance > 0 && capacity > 0 && capacity <= 65536);
        let namespace = next_namespace(&NEXT_RUNTIME).expect("runtime identity space exhausted");
        let (sender, receiver) = mpsc::channel();
        let shared = Arc::new(Shared {
            namespace,
            registry: Mutex::new(Registry::new(capacity)),
            sender,
            rejected: AtomicUsize::new(0),
        });
        let worker_shared = shared.clone();
        let worker =
            std::thread::spawn(move || worker(receiver, allowance, worker_shared, started));
        Self {
            client: RuntimeClient { shared },
            worker: Some(worker),
        }
    }
    pub fn client(&self) -> RuntimeClient {
        self.client.clone()
    }
    pub fn start(&self) {
        let registry = self.client.shared.registry.lock().unwrap();
        if registry.accepting {
            self.client
                .shared
                .sender
                .send(Command::Start)
                .expect("worker stopped");
        }
    }
    pub fn spawn(&self, program: Program, words: Vec<u64>) -> Result<ProcessHandle, SpawnError> {
        self.client.spawn(program, words)
    }
    pub fn spawn_input(&self, program: Program, input: Input) -> Result<ProcessHandle, SpawnError> {
        self.client.spawn_input(program, input)
    }
    pub fn spawn_to(
        &self,
        program: Program,
        input: Input,
        target: ProcessId,
    ) -> Result<ProcessHandle, SpawnError> {
        self.client.spawn_to(program, input, target)
    }
    pub fn shutdown(mut self) {
        self.join();
    }
    fn join(&mut self) {
        if let Some(worker) = self.worker.take() {
            self.client.close();
            worker.join().expect("worker failed");
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.join();
    }
}

pub(super) fn request_kill(metrics: &Metrics) -> KillReceipt {
    let _claim = metrics.lifecycle.lock().unwrap();
    if metrics.terminal.load(Ordering::Acquire) != 0 {
        return KillReceipt::TooLate;
    }
    if metrics.kill_event.load(Ordering::Acquire) != 0 {
        return KillReceipt::AlreadyRequested;
    }
    metrics.kill_event.store(event(), Ordering::Release);
    mailbox::notify(metrics);
    KillReceipt::Accepted
}

fn stop(task: &Task, reason: u8, shared: &Shared) -> LinkedList<Envelope> {
    assert!(!task.metrics.polling.load(Ordering::Acquire));
    let mut registry = shared.registry.lock().unwrap();
    let _claim = task.metrics.lifecycle.lock().unwrap();
    assert_eq!(task.metrics.terminal.load(Ordering::Acquire), 0);
    let reason = if task.metrics.kill_event.load(Ordering::Acquire) != 0 {
        2
    } else {
        reason
    };
    let pending = task.metrics.mailbox.lock().unwrap().close();
    registry.remove(task.id, &task.metrics);
    task.metrics.stop_event.store(event(), Ordering::Release);
    task.metrics.terminal.store(reason, Ordering::Release);
    pending
}
pub(super) struct Cleanup {
    metrics: Arc<Metrics>,
    pending: LinkedList<Envelope>,
}
impl Cleanup {
    pub(super) fn step(&mut self) -> bool {
        self.metrics.cleanup_batches.fetch_add(1, Ordering::Relaxed);
        for _ in 0..8 {
            if let Some(envelope) = self.pending.pop_front() {
                drop(envelope);
                self.metrics
                    .cleanup_messages
                    .fetch_add(1, Ordering::Relaxed);
            } else {
                break;
            }
        }
        if self.pending.is_empty() {
            self.metrics.cleanup_event.store(event(), Ordering::Release);
            true
        } else {
            false
        }
    }
}
fn cleanup(task: Task, pending: LinkedList<Envelope>) -> Cleanup {
    let metrics = task.metrics.clone();
    drop(task); // Selected payloads and native frame owners drop outside locks.
    Cleanup { metrics, pending }
}
pub(super) fn retire(task: Task, reason: u8, shared: &Shared) -> Cleanup {
    let pending = stop(&task, reason, shared);
    cleanup(task, pending)
}

#[cfg(test)]
mod tests;
