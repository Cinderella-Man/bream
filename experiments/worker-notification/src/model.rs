#![forbid(unsafe_code)]
//! Two-thread projections of queue enrollment and notification, not a scheduler.
use loom::sync::{Arc, Condvar, Mutex};
use loom::thread;
use std::sync::atomic::{AtomicUsize, Ordering};

static STARTED: AtomicUsize = AtomicUsize::new(0);

#[derive(Default)]
struct State {
    queued: usize,
    closed: bool,
    waiting: bool,
}

fn check(kind: u8) {
    let state = Arc::new((Mutex::new(State::default()), Condvar::new()));
    let worker = thread::current();
    let sender_state = state.clone();
    let sender = thread::spawn(move || {
        // One admitted item plus closed state: closing must still drain the item.
        let mut q = sender_state.0.lock().unwrap();
        q.queued += 1;
        q.closed = true;
        let wake = q.waiting;
        q.waiting = false;
        drop(q);
        if wake {
            if kind == 0 {
                sender_state.1.notify_one();
            } else {
                worker.unpark();
            }
        }
    });
    let mut consumed = 0;
    loop {
        let mut q = state.0.lock().unwrap();
        if q.queued > 0 {
            q.queued -= 1;
            q.waiting = false;
            consumed += 1;
        } else if q.closed {
            break;
        } else {
            if kind == 2 {
                // Broken: publication may happen after checking but before enrollment.
                drop(q);
                q = state.0.lock().unwrap();
            }
            q.waiting = true;
            if kind == 0 {
                drop(state.1.wait(q).unwrap());
            } else {
                drop(q);
                if kind == 3 {
                    // Broken: unrelated parking can consume the sole sticky token.
                    thread::park();
                }
                thread::park();
            }
        }
    }
    sender.join().unwrap();
    assert_eq!(consumed, 1, "WORK_NOT_CONSERVED");
}

fn main() {
    let name = std::env::args().nth(1).expect("model name");
    let kind = match name.as_str() {
        "condvar_locked" => 0,
        "park_locked" => 1,
        "park_split_enrollment" => 2,
        "park_token_consumed" => 3,
        _ => panic!("unknown model"),
    };
    let mut builder = loom::model::Builder::new();
    builder.max_threads = 2;
    builder.max_branches = 1000;
    builder.max_permutations = None;
    builder.max_duration = None;
    builder.preemption_bound = None;
    builder.checkpoint_file = None;
    builder.expect_explicit_explore = false;
    builder.location = true;
    builder.log = false;
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("MODEL_EXECUTIONS_STARTED={}", STARTED.load(Ordering::Relaxed));
        hook(info);
    }));
    builder.check(move || {
        STARTED.fetch_add(1, Ordering::Relaxed);
        check(kind);
    });
    println!("model={name} completed=true executions_started={}", STARTED.load(Ordering::Relaxed));
}
