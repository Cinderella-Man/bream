//! Disposable mechanism proof; the private executor is not a public safe API.
#![forbid(unsafe_code)]
use bream_probe_macros::process;
use bream_probe_runtime::{Metrics, ProcessContext};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

// Process source has no awaits, yields, runtime references or custom Drop.
#[process]
fn infinite_process() {
    let _owned = owned_buffer(4096);
    let mut counter: u64 = 0;
    loop {
        counter += 1;
        observe(counter);
        continue;
    }
}

#[process]
fn empty_spin() {
    loop {}
}

#[process]
fn survivor() {
    let mut warmup: u64 = 0;
    while warmup < 4 {
        warmup += 1;
    }
    for step in 0..64 {
        if step < 2 {
            observe(step + 1);
            continue;
        }
        observe(step + 1);
    }
}

struct NoopWake;
impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

type ResearchFuture = Pin<Box<dyn Future<Output = ()>>>;

fn main() {
    let worker = std::thread::current().id();
    let polling = Arc::new(AtomicBool::new(false));
    let metrics = [Arc::new(Metrics::default()), Arc::new(Metrics::default())];
    // These two constructors are macro-generated. The private executor accepts
    // ordinary Future internally; that is explicitly NOT an advertised process API.
    let infinite = infinite_process(ProcessContext::new(metrics[0].clone(), polling.clone()));
    let finite = survivor(ProcessContext::new(metrics[1].clone(), polling.clone()));
    let future_sizes = [
        std::mem::size_of_val(&infinite),
        std::mem::size_of_val(&finite),
    ];
    let mut slots: [Option<ResearchFuture>; 2] = [Some(Box::pin(infinite)), Some(Box::pin(finite))];
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    // Even a completely empty authored loop must give control back. There are
    // no progress builtins here that could be mistaken for cooperative yields.
    let mut empty = Box::pin(empty_spin(ProcessContext::new(
        Arc::new(Metrics::default()),
        polling.clone(),
    )));
    for _ in 0..2 {
        polling.store(true, Ordering::SeqCst);
        let outcome = empty.as_mut().poll(&mut context);
        polling.store(false, Ordering::SeqCst);
        assert_eq!(outcome, Poll::Pending);
    }
    assert!(!polling.load(Ordering::SeqCst));
    drop(empty);
    let mut polls = [0u64; 2];
    let mut completed = [false; 2];
    let mut killed = false;
    let mut kill_observed_after_pending = false;
    let mut survivor_at_kill = 0;
    for _round in 0..256 {
        for index in 0..2 {
            if slots[index].is_none() {
                continue;
            }
            assert_eq!(std::thread::current().id(), worker);
            let outcome = {
                polling.store(true, Ordering::SeqCst);
                let outcome = slots[index].as_mut().unwrap().as_mut().poll(&mut context);
                polling.store(false, Ordering::SeqCst);
                outcome
            };
            polls[index] += 1;
            if outcome == Poll::Ready(()) {
                completed[index] = true;
                drop(slots[index].take());
            }
            if index == 0 && !killed && metrics[0].progress.load(Ordering::SeqCst) >= 20 {
                assert_eq!(outcome, Poll::Pending);
                assert!(!polling.load(Ordering::SeqCst));
                assert_eq!(metrics[0].live_bytes.load(Ordering::SeqCst), 4096);
                survivor_at_kill = metrics[1].progress.load(Ordering::SeqCst);
                assert!(survivor_at_kill > 0 && survivor_at_kill < 64);
                kill_observed_after_pending = true;
                // Cancellation occurs on the executor stack, AFTER poll returned.
                // Ordinary Rust Drop destroys generated future fields and buffers.
                drop(slots[0].take());
                killed = true;
                assert_eq!(metrics[0].live_bytes.load(Ordering::SeqCst), 0);
                assert_eq!(metrics[0].drops.load(Ordering::SeqCst), 1);
                assert_eq!(metrics[0].drops_during_poll.load(Ordering::SeqCst), 0);
            }
        }
        if slots.iter().all(Option::is_none) {
            break;
        }
    }
    let survivor_final = metrics[1].progress.load(Ordering::SeqCst);
    assert!(killed && kill_observed_after_pending && !completed[0] && completed[1]);
    assert_eq!(survivor_final, 64);
    assert_eq!(metrics[0].allocated_bytes.load(Ordering::SeqCst), 4096);
    assert_eq!(metrics[0].freed_bytes.load(Ordering::SeqCst), 4096);
    assert!(slots.iter().all(Option::is_none));
    println!("{{\"passed\":true,\"task_workers\":1,\"worker_thread\":\"{:?}\",\"polls\":[{},{}],\"killed_after_pending\":{},\"infinite_process_completed\":{},\"infinite_progress_at_kill\":{},\"survivor_progress_at_kill\":{},\"survivor_final_progress\":{},\"allocated_bytes\":{},\"freed_bytes\":{},\"live_bytes_after_drop\":{},\"buffer_drops\":{},\"buffer_drops_during_poll\":{},\"custom_context_switching\":false,\"generated_future_bytes\":[{},{}],\"empty_loop_pending_polls\":2,\"empty_loop_dropped_after_poll\":true}}",
        worker, polls[0], polls[1], kill_observed_after_pending, completed[0],
        metrics[0].progress.load(Ordering::SeqCst), survivor_at_kill, survivor_final,
        metrics[0].allocated_bytes.load(Ordering::SeqCst), metrics[0].freed_bytes.load(Ordering::SeqCst),
        metrics[0].live_bytes.load(Ordering::SeqCst), metrics[0].drops.load(Ordering::SeqCst),
        metrics[0].drops_during_poll.load(Ordering::SeqCst), future_sizes[0], future_sizes[1]);
}
