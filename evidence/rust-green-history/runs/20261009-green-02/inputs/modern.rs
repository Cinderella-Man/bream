#![forbid(unsafe_code)]
mod registration;

use registration::Registration;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    slot: u32,
    generation: u32,
}

struct Buffer {
    bytes: Box<[u8]>,
    drops: Arc<AtomicUsize>,
}
impl Drop for Buffer {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
struct Completion {
    owner: Identity,
    buffer: Buffer,
}
struct Actor {
    identity: Identity,
    result: mpsc::Receiver<Completion>,
    drops: Arc<AtomicUsize>,
}
impl Drop for Actor {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
struct Operation {
    complete: mpsc::Sender<()>,
    driver: JoinHandle<bool>,
}

// The driver owns the buffer throughout the outstanding operation. Its callback
// never borrows the actor's frame. A real native adapter would have to enforce
// this lifetime across its FFI contract; this probe contains no FFI or real I/O.
fn submit(
    owner: Identity,
    drops: &Arc<AtomicUsize>,
    destination: mpsc::Sender<Completion>,
) -> Operation {
    let mut buffer = Buffer {
        bytes: vec![0; 4].into_boxed_slice(),
        drops: drops.clone(),
    };
    let (accepted_tx, accepted_rx) = mpsc::channel();
    let (complete, gate) = mpsc::channel();
    let driver = thread::spawn(move || {
        accepted_tx.send(()).unwrap();
        gate.recv().unwrap();
        buffer.bytes[0] = 7;
        destination.send(Completion { owner, buffer }).is_ok()
    });
    accepted_rx.recv().unwrap();
    Operation { complete, driver }
}
fn settle(operation: Operation) -> bool {
    operation.complete.send(()).unwrap();
    operation.driver.join().unwrap()
}
fn owned_operation(cancel: bool) {
    let drops = Arc::new(AtomicUsize::new(0));
    let actor_drops = Arc::new(AtomicUsize::new(0));
    let identity = Identity {
        slot: 1,
        generation: 1,
    };
    let (tx, rx) = mpsc::channel();
    let actor = Actor {
        identity,
        result: rx,
        drops: actor_drops.clone(),
    };
    let operation = submit(identity, &drops, tx);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    if cancel {
        drop(actor);
        assert_eq!(actor_drops.load(Ordering::SeqCst), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        // A separate application step is possible after frame destruction,
        // before the host permits the outstanding callback to finish.
        let mut peer = 16;
        peer += 1;
        assert_eq!(peer, 17);
        assert!(!settle(operation)); // receiver gone: undelivered buffer drops
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        println!(
            "{{\"case\":\"owned_cancel\",\"actor_drops\":1,\"buffer_retained_after_stop\":true,\"peer_before_completion\":17,\"buffer_drops\":1}}"
        );
    } else {
        assert!(settle(operation));
        let completion = actor.result.recv().unwrap();
        assert!(completion.owner == actor.identity);
        assert_eq!(completion.buffer.bytes[0], 7);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(completion);
        drop(actor);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(actor_drops.load(Ordering::SeqCst), 1);
        println!(
            "{{\"case\":\"owned_complete\",\"value\":7,\"actor_drops\":1,\"buffer_drops\":1}}"
        );
    }
}

fn reused_identity() {
    let drops = Arc::new(AtomicUsize::new(0));
    let old = Identity {
        slot: 1,
        generation: 1,
    };
    let new = Identity {
        slot: 1,
        generation: 2,
    };
    let (tx, router) = mpsc::channel();
    let operation = submit(old, &drops, tx);
    // The routing slot now names a different incarnation; the external
    // operation still owns its buffer and the original full identity.
    let mut replacement_value = 99;
    assert!(settle(operation));
    let completion = router.recv().unwrap();
    let accepted = completion.owner == new;
    if accepted {
        replacement_value = u32::from(completion.buffer.bytes[0]);
    }
    assert!(!accepted);
    drop(completion);
    assert_eq!(replacement_value, 99);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    println!(
        "{{\"case\":\"reused_identity\",\"stale_rejected\":true,\"replacement_value\":99,\"buffer_drops\":1}}"
    );
}

#[derive(Clone, Copy)]
enum Event {
    Wake,
    ResumeAttempt,
    CallbackDone,
}

fn wake_registration() {
    use Event::*;
    // All linear extensions after publication, with wake before resume attempt.
    // CallbackDone may occur in any of the three positions. Abstract sequential
    // model only: it does not validate atomics or a production scheduler.
    let schedules = [
        [Wake, ResumeAttempt, CallbackDone],
        [Wake, CallbackDone, ResumeAttempt],
        [CallbackDone, Wake, ResumeAttempt],
    ];
    let mut naive_violations = 0;
    let mut barrier_violations = 0;
    let mut completed = 0;
    for schedule in schedules {
        let mut active = true;
        let mut winner = false;
        let mut deferred = false;
        let mut resumed = false;
        for event in schedule {
            match event {
                Wake => winner = true,
                ResumeAttempt => {
                    assert!(winner);
                    if active {
                        naive_violations += 1;
                        deferred = true;
                    } else {
                        resumed = true;
                    }
                }
                CallbackDone => {
                    active = false;
                    if deferred {
                        resumed = true;
                    }
                }
            }
            if resumed && active {
                barrier_violations += 1;
            }
        }
        assert!(resumed && !active);
        completed += 1;
    }
    assert_eq!(naive_violations, 1);
    assert_eq!(barrier_violations, 0);
    let mut environment = 0;
    let mut registration = Registration::new(&mut environment);
    registration.touch();
    let _publishable_ticket = registration.finish();
    assert_eq!(environment, 1);
    println!(
        "{{\"case\":\"wake_registration\",\"schedules\":{},\"naive_violations\":{},\"barrier_violations\":{},\"consuming_registration\":true}}",
        completed, naive_violations, barrier_violations
    );
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("owned_cancel") => owned_operation(true),
        Some("owned_complete") => owned_operation(false),
        Some("reused_identity") => reused_identity(),
        Some("wake_registration") => wake_registration(),
        _ => panic!("unknown case"),
    }
}
