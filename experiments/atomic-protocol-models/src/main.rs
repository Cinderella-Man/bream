#![forbid(unsafe_code)]
//! Small protocol projections. No Bream scheduler, raw-pointer lifetime or VM model.

use loom::sync::atomic::{fence, AtomicBool, AtomicUsize, Ordering};
use loom::sync::{Arc, Mutex};
use loom::thread;
use std::sync::atomic::{AtomicUsize as HostCounter, Ordering as HostOrdering};

// Observation only: never participates in a modeled protocol decision.
static STARTED: HostCounter = HostCounter::new(0);

fn publication(store: Ordering, load: Ordering) {
    let shared = Arc::new((AtomicUsize::new(0), AtomicBool::new(false)));
    let writer = shared.clone();
    let thread = thread::spawn(move || {
        writer.0.store(42, Ordering::Relaxed);
        writer.1.store(true, store);
    });
    if shared.1.load(load) {
        let payload = shared.0.load(Ordering::Relaxed);
        assert_eq!(payload, 42, "PUBLICATION_STALE ready=true payload={payload}");
    }
    thread.join().unwrap();
}

struct Owners {
    refs: AtomicUsize,
    writes: [AtomicUsize; 2],
    finalizers: AtomicUsize,
}

fn release_owner(state: &Owners, owner: usize, acquire_final: bool) {
    state.writes[owner].store(1, Ordering::Relaxed);
    if state.refs.fetch_sub(1, Ordering::Release) == 1 {
        if acquire_final {
            fence(Ordering::Acquire);
        }
        let a = state.writes[0].load(Ordering::Relaxed);
        let b = state.writes[1].load(Ordering::Relaxed);
        assert_eq!((a, b), (1, 1), "LAST_OWNER_STALE writes=({a},{b})");
        state.finalizers.fetch_add(1, Ordering::Relaxed);
    }
}

fn final_owner(acquire_final: bool) {
    let state = Arc::new(Owners {
        refs: AtomicUsize::new(2),
        writes: [AtomicUsize::new(0), AtomicUsize::new(0)],
        finalizers: AtomicUsize::new(0),
    });
    let other = state.clone();
    let thread = thread::spawn(move || release_owner(&other, 0, acquire_final));
    release_owner(&state, 1, acquire_final);
    thread.join().unwrap();
    assert_eq!(state.finalizers.load(Ordering::Relaxed), 1);
}

fn parking_locked() {
    // (message is available, receiver committed to waiting, wake obligation)
    let state = Arc::new(Mutex::new((false, false, false)));
    let publisher = state.clone();
    let sender = thread::spawn(move || {
        let mut s = publisher.lock().unwrap();
        s.0 = true;
        if s.1 {
            s.2 = true;
        }
    });
    {
        let mut s = state.lock().unwrap();
        if !s.0 {
            s.1 = true;
        }
    }
    sender.join().unwrap();
    let s = state.lock().unwrap();
    assert!(!s.1 || s.2, "PARKING_LOST_WAKE");
}

fn parking_separate_flags() {
    let state = Arc::new((AtomicBool::new(false), AtomicBool::new(false)));
    let publisher = state.clone();
    let sender = thread::spawn(move || {
        publisher.0.store(true, Ordering::Release);
        publisher.1.load(Ordering::Acquire)
    });
    state.1.store(true, Ordering::Release);
    let saw_message = state.0.load(Ordering::Acquire);
    let issued_wake = sender.join().unwrap();
    assert!(saw_message || issued_wake,
            "PARKING_LOST_WAKE saw_message={saw_message} issued_wake={issued_wake}");
}

fn main() {
    let name = std::env::args().nth(1).expect("model name");
    let run: fn() = match name.as_str() {
        "publication_release_acquire" => || publication(Ordering::Release, Ordering::Acquire),
        "publication_relaxed_store" => || publication(Ordering::Relaxed, Ordering::Acquire),
        "publication_relaxed_load" => || publication(Ordering::Release, Ordering::Relaxed),
        "final_owner_acquire_fence" => || final_owner(true),
        "final_owner_no_fence" => || final_owner(false),
        "parking_locked" => parking_locked,
        "parking_separate_acqrel" => parking_separate_flags,
        _ => panic!("unknown model: {name}"),
    };
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("MODEL_EXECUTIONS_STARTED={}", STARTED.load(HostOrdering::Relaxed));
        default_hook(info);
    }));
    let mut builder = loom::model::Builder::new();
    builder.max_threads = 2;
    builder.max_branches = 1_000;
    builder.max_permutations = None;
    builder.max_duration = None;
    builder.preemption_bound = None;
    builder.checkpoint_file = None;
    builder.expect_explicit_explore = false;
    builder.location = true;
    builder.log = false;
    builder.check(move || {
        STARTED.fetch_add(1, HostOrdering::Relaxed);
        run();
    });
    println!("model={name} completed=true executions_started={}", STARTED.load(HostOrdering::Relaxed));
}
