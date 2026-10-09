//! Bounded semantic probes, not a Bream implementation or a preemption test.
//! The driver cancels only after poll returns Pending. No arbitrary stack is freed.
#![forbid(unsafe_code)]

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::task::{Context, Poll, Waker};
use std::thread::{self, JoinHandle};

type Count = Arc<AtomicUsize>;

fn count() -> Count {
    Arc::new(AtomicUsize::new(0))
}

fn read(value: &Count) -> usize {
    value.load(Ordering::SeqCst)
}

fn require_send_static<T: Send + 'static>(_: &T) {}
fn require_send_sync_static<T: Send + Sync + 'static>(_: &T) {}

// Explicit suspension is a test gate, not proposed Bream author syntax.
struct Gate(bool);

impl Future for Gate {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            Poll::Pending
        }
    }
}

fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

struct Owned {
    bytes: Vec<u8>,
    drops: Count,
}

impl Owned {
    fn new(drops: Count) -> Self {
        Self {
            bytes: vec![2, 3, 5, 7],
            drops,
        }
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        self.bytes.clear();
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

fn emit(case: &str, fields: &[(&str, usize)]) {
    let fields = fields
        .iter()
        .map(|(key, value)| format!("\"{key}\":{value}"))
        .collect::<Vec<_>>()
        .join(",");
    println!("{{\"case\":\"{case}\",{fields}}}");
}

#[derive(Clone, Copy)]
struct Ledger {
    left: usize,
    right: usize,
}

impl Ledger {
    fn initial() -> Self {
        Self {
            left: 100,
            right: 100,
        }
    }

    fn total(self) -> usize {
        self.left + self.right
    }
}

fn owned_cancel() {
    let drops = count();
    let receipt = drops.clone();
    let future = async move {
        let _owned = Owned::new(receipt);
        let mut local = Ledger::initial();
        local.left -= 1;
        Gate(false).await;
        local.right += 1;
        local.total()
    };
    require_send_sync_static(&future);
    let mut actor = Box::pin(future);
    assert!(poll(actor.as_mut()).is_pending());
    assert_eq!(read(&drops), 0);
    drop(actor);
    let mut peer = Box::pin(async { Ledger::initial().total() });
    let Poll::Ready(total) = poll(peer.as_mut()) else {
        panic!("independent peer did not answer");
    };
    assert_eq!(total, 200);
    assert_eq!(read(&drops), 1);
    emit(
        "owned_cancel",
        &[("peer_total", total), ("frame_drops", read(&drops))],
    );
}

fn shared_update(complete: bool, atomic_commit: bool) {
    let state = Arc::new(Mutex::new(Ledger::initial()));
    let actor_state = state.clone();
    let drops = count();
    let receipt = drops.clone();
    let future = async move {
        let _owned = Owned::new(receipt);
        {
            let mut state = actor_state.lock().unwrap();
            state.left -= 1;
            if atomic_commit {
                state.right += 1;
            }
        } // No MutexGuard crosses the suspension point.
        Gate(false).await;
        if !atomic_commit {
            actor_state.lock().unwrap().right += 1;
        }
    };
    require_send_sync_static(&future);
    let mut actor = Box::pin(future);
    assert!(poll(actor.as_mut()).is_pending());
    if complete {
        assert!(poll(actor.as_mut()).is_ready());
    }
    drop(actor);
    // A real peer thread reads after the target frame has been destroyed.
    let peer = thread::spawn(move || {
        let unpoisoned = !state.is_poisoned();
        let snapshot = *state.try_lock().expect("cancellation retained a lock");
        (snapshot, unpoisoned)
    });
    let (snapshot, unpoisoned) = peer.join().unwrap();
    let expected = if complete || atomic_commit { 200 } else { 199 };
    assert_eq!(snapshot.total(), expected);
    assert!(unpoisoned);
    assert_eq!(read(&drops), 1);
    let case = if atomic_commit {
        "atomic_commit_cancel"
    } else if complete {
        "shared_complete"
    } else {
        "shared_cancel"
    };
    emit(
        case,
        &[
            ("peer_total", snapshot.total()),
            ("left", snapshot.left),
            ("right", snapshot.right),
            ("mutex_unpoisoned", usize::from(unpoisoned)),
            ("frame_drops", read(&drops)),
        ],
    );
}

struct Rollback {
    state: Arc<Mutex<Ledger>>,
    rollbacks: Count,
}

impl Drop for Rollback {
    fn drop(&mut self) {
        self.state.lock().unwrap().left += 1;
        self.rollbacks.fetch_add(1, Ordering::SeqCst);
    }
}

fn rollback_cancel() {
    let state = Arc::new(Mutex::new(Ledger::initial()));
    let actor_state = state.clone();
    let rollbacks = count();
    let receipt = rollbacks.clone();
    let future = async move {
        let _rollback = Rollback {
            state: actor_state.clone(),
            rollbacks: receipt,
        };
        actor_state.lock().unwrap().left -= 1;
        Gate(false).await;
    };
    require_send_sync_static(&future);
    let mut actor = Box::pin(future);
    assert!(poll(actor.as_mut()).is_pending());
    assert_eq!(state.lock().unwrap().total(), 199);
    drop(actor);
    let total = state.lock().unwrap().total();
    assert_eq!(total, 200);
    assert_eq!(read(&rollbacks), 1);
    emit(
        "rollback_cancel",
        &[("peer_total", total), ("rollbacks", read(&rollbacks))],
    );
}

enum ChildCommand {
    Work,
    Stop,
}

fn child(
    commands: mpsc::Receiver<ChildCommand>,
    replies: mpsc::Sender<usize>,
    ready: mpsc::Sender<()>,
    effects: Count,
    drops: Count,
) {
    let _owned = Owned::new(drops);
    ready.send(()).unwrap();
    while let Ok(command) = commands.recv() {
        match command {
            ChildCommand::Work => {
                let value = effects.fetch_add(1, Ordering::SeqCst) + 1;
                replies.send(value).unwrap();
            }
            ChildCommand::Stop => break,
        }
    }
}

fn child_survives() {
    let (commands, incoming) = mpsc::channel();
    let (replies, outgoing) = mpsc::channel();
    let (ready, started) = mpsc::channel();
    let (handle_sender, handle_receiver) = mpsc::channel();
    let effects = count();
    let child_effects = effects.clone();
    let child_drops = count();
    let child_receipt = child_drops.clone();
    let parent_drops = count();
    let parent_receipt = parent_drops.clone();
    let future = async move {
        let _owned = Owned::new(parent_receipt);
        let handle =
            thread::spawn(move || child(incoming, replies, ready, child_effects, child_receipt));
        // The harness retains the join handle solely to guarantee teardown.
        // No stop command is implicitly associated with dropping this future.
        handle_sender.send(handle).unwrap();
        Gate(false).await;
    };
    require_send_static(&future);
    let mut actor = Box::pin(future);
    assert!(poll(actor.as_mut()).is_pending());
    started.recv().unwrap();
    let handle: JoinHandle<()> = handle_receiver.recv().unwrap();
    assert_eq!(read(&effects), 0);
    drop(actor);
    assert_eq!(read(&parent_drops), 1);
    assert_eq!(read(&child_drops), 0);
    // This work is ordered strictly after parent frame destruction.
    commands.send(ChildCommand::Work).unwrap();
    let observed = outgoing.recv().unwrap();
    assert_eq!(observed, 1);
    commands.send(ChildCommand::Stop).unwrap();
    handle.join().unwrap();
    assert_eq!(read(&child_drops), 1);
    emit(
        "child_survives",
        &[
            ("parent_drops", read(&parent_drops)),
            ("child_effects_after_parent_drop", observed),
            ("child_drops_after_explicit_stop", read(&child_drops)),
        ],
    );
}

struct JoinedChild {
    commands: mpsc::Sender<ChildCommand>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for JoinedChild {
    fn drop(&mut self) {
        self.commands.send(ChildCommand::Stop).unwrap();
        self.handle.take().unwrap().join().unwrap();
    }
}

fn managed_child_cancel() {
    let (commands, incoming) = mpsc::channel();
    let command_probe = commands.clone();
    let (replies, _outgoing) = mpsc::channel();
    let (ready, started) = mpsc::channel();
    let effects = count();
    let child_effects = effects.clone();
    let child_drops = count();
    let child_receipt = child_drops.clone();
    let future = async move {
        let handle =
            thread::spawn(move || child(incoming, replies, ready, child_effects, child_receipt));
        let _child = JoinedChild {
            commands,
            handle: Some(handle),
        };
        Gate(false).await;
    };
    require_send_static(&future);
    let mut actor = Box::pin(future);
    assert!(poll(actor.as_mut()).is_pending());
    started.recv().unwrap();
    drop(actor);
    assert!(command_probe.send(ChildCommand::Work).is_err());
    assert_eq!(read(&effects), 0);
    assert_eq!(read(&child_drops), 1);
    emit(
        "managed_child_cancel",
        &[
            ("child_effects", read(&effects)),
            ("child_drops", read(&child_drops)),
            ("post_drop_command_rejected", 1),
        ],
    );
}

struct GatedDrop {
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    finished: Count,
}

impl Drop for GatedDrop {
    fn drop(&mut self) {
        self.entered.send(()).unwrap();
        self.release.recv().unwrap();
        self.finished.fetch_add(1, Ordering::SeqCst);
    }
}

fn cleanup_case(blocking: bool) {
    let finished = count();
    let (peer_done, peer_reply) = mpsc::channel();
    let blocked;
    if blocking {
        let (entered, entry) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let receipt = finished.clone();
        let future = async move {
            let _drop = GatedDrop {
                entered,
                release: released,
                finished: receipt,
            };
            Gate(false).await;
        };
        require_send_static(&future);
        let mut actor = Box::pin(future);
        assert!(poll(actor.as_mut()).is_pending());
        let worker = thread::spawn(move || {
            drop(actor);
            // This peer's next turn shares the thread used for cleanup.
            let mut peer = Box::pin(async { 17usize });
            let Poll::Ready(value) = poll(peer.as_mut()) else {
                panic!("peer pending")
            };
            peer_done.send(value).unwrap();
        });
        entry.recv().unwrap();
        assert_eq!(read(&finished), 0);
        blocked = matches!(peer_reply.try_recv(), Err(mpsc::TryRecvError::Empty));
        assert!(blocked);
        // Channel gating, not elapsed time, establishes why cleanup is blocked.
        release.send(()).unwrap();
        assert_eq!(peer_reply.recv().unwrap(), 17);
        worker.join().unwrap();
    } else {
        let receipt = finished.clone();
        let future = async move {
            let _owned = Owned::new(receipt);
            Gate(false).await;
        };
        require_send_static(&future);
        let mut actor = Box::pin(future);
        assert!(poll(actor.as_mut()).is_pending());
        let worker = thread::spawn(move || {
            drop(actor);
            let mut peer = Box::pin(async { 17usize });
            let Poll::Ready(value) = poll(peer.as_mut()) else {
                panic!("peer pending")
            };
            peer_done.send(value).unwrap();
        });
        assert_eq!(peer_reply.recv().unwrap(), 17);
        worker.join().unwrap();
        blocked = false;
    }
    assert_eq!(read(&finished), 1);
    emit(
        if blocking {
            "blocking_drop"
        } else {
            "bounded_drop"
        },
        &[
            ("peer_blocked_at_destructor_gate", usize::from(blocked)),
            ("cleanup_finished", read(&finished)),
            ("peer_reply", 17),
        ],
    );
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("owned_cancel") => owned_cancel(),
        Some("shared_cancel") => shared_update(false, false),
        Some("shared_complete") => shared_update(true, false),
        Some("atomic_commit_cancel") => shared_update(false, true),
        Some("rollback_cancel") => rollback_cancel(),
        Some("child_survives") => child_survives(),
        Some("managed_child_cancel") => managed_child_cancel(),
        Some("blocking_drop") => cleanup_case(true),
        Some("bounded_drop") => cleanup_case(false),
        _ => panic!("provide a known semantic case name"),
    }
}
