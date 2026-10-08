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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    OriginNotLive,
    MonitorExhausted,
}
#[derive(Clone, Copy)]
enum InitialRelationship {
    Link,
    Monitor,
    LinkMonitor,
}
impl InitialRelationship {
    fn linked(self) -> bool {
        matches!(self, Self::Link | Self::LinkMonitor)
    }
    fn monitored(self) -> bool {
        matches!(self, Self::Monitor | Self::LinkMonitor)
    }
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
    pub(super) clock: Arc<timer::Clock>,
    pub(super) monitors: Arc<monitor::Hub>,
    pub(super) links: Arc<links::Hub>,
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
    fn from_owned(
        program: Program,
        input: OwnedWords,
        tag: u64,
        argument: u64,
        shared: &Arc<Shared>,
        origin: ProcessId,
    ) -> Self {
        // The allocation receipt follows the buffer. Child diagnostic metadata
        // does not reattribute storage or retain the producer's execution.
        let metrics = Arc::new(Metrics::default());
        Self::frame(program, input, tag, argument, shared, Some(origin), metrics)
    }
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
        metrics
            .allocation
            .live_bytes
            .store(input.words.capacity() as u64 * 8, Ordering::Relaxed);
        let owned = OwnedWords {
            words: input.words,
            allocation: metrics.allocation.clone(),
        };
        Ok(Self::frame(
            program,
            owned,
            input.tag,
            input.argument,
            shared,
            target,
            metrics,
        ))
    }
    fn frame(
        program: Program,
        input: OwnedWords,
        tag: u64,
        argument: u64,
        shared: &Arc<Shared>,
        target: Option<ProcessId>,
        metrics: Arc<Metrics>,
    ) -> Self {
        metrics.runtime.set(Arc::downgrade(shared)).unwrap();
        let budget = Arc::new(Budget {
            remaining: AtomicU32::new(0),
        });
        let context = ProcessContext {
            input: Some(input),
            target,
            tag,
            argument,
            budget: budget.clone(),
            metrics: metrics.clone(),
        };
        let frame = (program.make)(context);
        Self {
            frame,
            metrics,
            budget,
        }
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
    /// Inject monotonic logical time. No host-clock reading or waiting is performed.
    pub fn advance_to(&self, now: u128) -> Result<(), ClockError> {
        let registry = self.shared.registry.lock().unwrap();
        if !registry.accepting {
            return Err(ClockError::Closed);
        }
        self.shared.clock.advance_to(now)?;
        self.shared
            .sender
            .send(Command::ClockAdvanced)
            .map_err(|_| ClockError::Closed)
    }
    pub fn clock_snapshot(&self) -> ClockSnapshot {
        self.shared.clock.snapshot()
    }
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
    /// Ordinary local send consumes this message's share even for absent IDs.
    pub fn send_shared(&self, id: ProcessId, envelope: SharedEnvelope) {
        if let Ok(handle) = self.lookup(id) {
            mailbox::deliver_event(&handle.metrics, Event::Shared(envelope));
        }
    }
    fn publish(&self, prepared: Prepared) -> Result<ProcessHandle, SpawnError> {
        self.publish_initial(prepared, None)
            .map(|(handle, _)| handle)
    }
    fn publish_initial(
        &self,
        prepared: Prepared,
        initial: Option<(ProcessId, InitialRelationship)>,
    ) -> Result<(ProcessHandle, Option<MonitorRef>), SpawnError> {
        let mut registry = self.shared.registry.lock().unwrap();
        let origin = initial.and_then(|(id, _)| {
            (id.runtime == self.shared.namespace)
                .then(|| registry.slots.get(id.slot))
                .flatten()
                .filter(|slot| slot.generation == id.generation)
                .and_then(|slot| slot.record.clone())
        });
        let rejection = if !registry.accepting {
            Some(SpawnErrorKind::Closed)
        } else if initial.is_some() && origin.is_none() {
            Some(SpawnErrorKind::OriginNotLive)
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
        // Install initial topology before dispatch. Retirement takes this same
        // registry guard, so neither endpoint can detach a half-published edge.
        let mut reference = None;
        if let Some((parent, mode)) = initial {
            let owner = origin.as_ref().unwrap();
            if mode.monitored() {
                match self.shared.monitors.create(parent, owner, id, true) {
                    Ok((created, work)) => {
                        assert!(!work);
                        reference = Some(created);
                    }
                    Err(MonitorError::Exhausted) => {
                        registry.remove(id, &metrics);
                        drop(registry);
                        return Err(prepared.reject(SpawnErrorKind::MonitorExhausted, &self.shared));
                    }
                    Err(_) => unreachable!("validated monitor origin"),
                }
            }
            if mode.linked() {
                assert!(!self
                    .shared
                    .links
                    .create(parent, owner, id, Some(&metrics))
                    .unwrap());
            }
        }
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
            if let Some((parent, mode)) = initial {
                if mode.linked() {
                    self.shared.links.unlink(parent, id);
                }
                if let Some(reference) = reference {
                    self.shared.monitors.cancel(parent, reference);
                }
            }
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
        Ok((ProcessHandle { id, metrics }, reference))
    }
    fn spawn_initial(
        &self,
        origin: ProcessId,
        program: Program,
        input: Input,
        mode: InitialRelationship,
    ) -> Result<(ProcessHandle, Option<MonitorRef>), SpawnError> {
        let prepared = Prepared::build(program, input, &self.shared, Some(origin))?;
        self.publish_initial(prepared, Some((origin, mode)))
    }
    pub fn spawn_link(
        &self,
        origin: ProcessId,
        program: Program,
        input: Input,
    ) -> Result<ProcessHandle, SpawnError> {
        self.spawn_initial(origin, program, input, InitialRelationship::Link)
            .map(|(handle, _)| handle)
    }
    pub fn spawn_monitor(
        &self,
        origin: ProcessId,
        program: Program,
        input: Input,
    ) -> Result<(ProcessHandle, MonitorRef), SpawnError> {
        self.spawn_initial(origin, program, input, InitialRelationship::Monitor)
            .map(|(handle, reference)| (handle, reference.unwrap()))
    }
    pub fn spawn_link_monitor(
        &self,
        origin: ProcessId,
        program: Program,
        input: Input,
    ) -> Result<(ProcessHandle, MonitorRef), SpawnError> {
        self.spawn_initial(origin, program, input, InitialRelationship::LinkMonitor)
            .map(|(handle, reference)| (handle, reference.unwrap()))
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
    /// Trusted host control. The observer must be live; a foreign or stale
    /// target behaves as absent. Applications use the checked context operation.
    pub fn monitor(
        &self,
        observer: ProcessId,
        target: ProcessId,
    ) -> Result<MonitorRef, MonitorError> {
        let registry = self.shared.registry.lock().unwrap();
        let lookup = |id: ProcessId| {
            if id.runtime != self.shared.namespace {
                return None;
            }
            registry
                .slots
                .get(id.slot)
                .filter(|slot| slot.generation == id.generation)
                .and_then(|slot| slot.record.as_ref())
        };
        let origin = lookup(observer).ok_or(MonitorError::ObserverNotLive)?;
        let (reference, work) =
            self.shared
                .monitors
                .create(observer, origin, target, lookup(target).is_some())?;
        if work && self.shared.monitors.queue_wake() {
            let _ = self.shared.sender.send(Command::MonitorWork);
        }
        Ok(reference)
    }
    /// Plain cancellation never flushes a queued Down. This bool is the
    /// origin-removal diagnostic (the reference's info option).
    pub fn demonitor_info(&self, observer: ProcessId, reference: MonitorRef) -> bool {
        self.shared.monitors.cancel(observer, reference)
    }
    pub fn active_monitors(&self) -> usize {
        self.shared.monitors.active()
    }
    /// Shared local relationship publication is serialized with both retirements.
    pub fn link(&self, origin: ProcessId, target: ProcessId) -> Result<(), LinkError> {
        let registry = self.shared.registry.lock().unwrap();
        let lookup = |id: ProcessId| {
            if id.runtime != self.shared.namespace {
                return None;
            }
            registry
                .slots
                .get(id.slot)
                .filter(|slot| slot.generation == id.generation)
                .and_then(|slot| slot.record.as_ref())
        };
        let owner = lookup(origin).ok_or(LinkError::OriginNotLive)?;
        let work = self
            .shared
            .links
            .create(origin, owner, target, lookup(target))?;
        if work && self.shared.links.queue_wake() {
            let _ = self.shared.sender.send(Command::LinkWork);
        }
        Ok(())
    }
    pub fn unlink(&self, origin: ProcessId, target: ProcessId) {
        self.shared.links.unlink(origin, target);
    }
    pub fn set_trap_exit(&self, id: ProcessId, enabled: bool) -> Result<bool, LookupError> {
        let handle = self.lookup(id)?;
        Ok(self.shared.links.set_trap(&handle.metrics, enabled))
    }
    pub fn active_links(&self) -> usize {
        self.shared.links.active()
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

impl ProcessContext {
    fn spawn_child(
        &self,
        program: Program,
        words: OwnedWords,
        tag: u64,
        argument: u64,
        mode: InitialRelationship,
    ) -> (ProcessId, Option<MonitorRef>) {
        let shared = self
            .metrics
            .runtime
            .get()
            .and_then(Weak::upgrade)
            .expect("live runtime");
        let origin = self.self_id();
        let prepared = Prepared::from_owned(program, words, tag, argument, &shared, origin);
        match (RuntimeClient { shared }).publish_initial(prepared, Some((origin, mode))) {
            Ok((handle, reference)) => (handle.id(), reference),
            Err(error) => panic!("child spawn failed: {:?}", error.kind),
        }
    }
    pub fn spawn_link(
        &self,
        program: Program,
        words: OwnedWords,
        tag: u64,
        argument: u64,
    ) -> ProcessId {
        self.spawn_child(program, words, tag, argument, InitialRelationship::Link)
            .0
    }
    pub fn spawn_monitor(
        &self,
        program: Program,
        words: OwnedWords,
        tag: u64,
        argument: u64,
    ) -> Spawned {
        let (pid, reference) =
            self.spawn_child(program, words, tag, argument, InitialRelationship::Monitor);
        Spawned {
            pid,
            reference: reference.unwrap(),
        }
    }
    pub fn spawn_link_monitor(
        &self,
        program: Program,
        words: OwnedWords,
        tag: u64,
        argument: u64,
    ) -> Spawned {
        let (pid, reference) = self.spawn_child(
            program,
            words,
            tag,
            argument,
            InitialRelationship::LinkMonitor,
        );
        Spawned {
            pid,
            reference: reference.unwrap(),
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
            clock: Arc::new(timer::Clock::default()),
            monitors: Arc::new(monitor::Hub::default()),
            links: Arc::new(links::Hub::default()),
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
    if metrics.terminal.load(Ordering::Acquire) != 0
        || metrics.pending_exit.load(Ordering::Acquire) != 0
    {
        return KillReceipt::TooLate;
    }
    if metrics.kill_event.load(Ordering::Acquire) != 0 {
        return KillReceipt::AlreadyRequested;
    }
    metrics.kill_event.store(event(), Ordering::Release);
    mailbox::notify(metrics);
    KillReceipt::Accepted
}

struct Stopped {
    messages: LinkedList<Event>,
    relationships: monitor::Retiring,
    links: links::Retiring,
}
fn stop(task: &Task, reason: u8, shared: &Shared) -> Stopped {
    assert!(!task.metrics.polling.load(Ordering::Acquire));
    let mut registry = shared.registry.lock().unwrap();
    let _claim = task.metrics.lifecycle.lock().unwrap();
    assert_eq!(task.metrics.terminal.load(Ordering::Acquire), 0);
    let forced = task.metrics.kill_event.load(Ordering::Acquire) != 0;
    let inherited = task.metrics.pending_exit.load(Ordering::Acquire);
    let reason = if forced {
        2
    } else if inherited != 0 {
        3
    } else {
        reason
    };
    let exit_reason = if forced {
        2
    } else if inherited != 0 {
        inherited
    } else {
        u64::from(reason)
    };
    let pending = task.metrics.mailbox.lock().unwrap().close();
    registry.remove(task.id, &task.metrics);
    task.metrics.stop_event.store(event(), Ordering::Release);
    task.metrics
        .exit_reason
        .store(exit_reason, Ordering::Release);
    task.metrics.terminal.store(reason, Ordering::Release);
    let relationships = shared.monitors.detach(task.id, exit_reason as u8);
    let links = shared.links.detach(task.id, exit_reason);
    Stopped {
        messages: pending,
        relationships,
        links,
    }
}
pub(super) struct Cleanup {
    metrics: Arc<Metrics>,
    pending: LinkedList<Event>,
    relationships: monitor::Retiring,
    links: links::Retiring,
}
impl Cleanup {
    pub(super) fn step(&mut self) -> bool {
        self.metrics.cleanup_batches.fetch_add(1, Ordering::Relaxed);
        let (links_done, used) = self.links.step(8);
        let relationships_done = links_done && self.relationships.step(8 - used);
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
        if self.pending.is_empty() && relationships_done {
            self.metrics.cleanup_event.store(event(), Ordering::Release);
            true
        } else {
            false
        }
    }
}
fn cleanup(task: Task, stopped: Stopped) -> Cleanup {
    let metrics = task.metrics.clone();
    drop(task); // Selected payloads and native frame owners drop outside locks.
    Cleanup {
        metrics,
        pending: stopped.messages,
        relationships: stopped.relationships,
        links: stopped.links,
    }
}
pub(super) fn retire(task: Task, reason: u8, shared: &Shared) -> Cleanup {
    let pending = stop(&task, reason, shared);
    cleanup(task, pending)
}

#[cfg(test)]
mod tests;
