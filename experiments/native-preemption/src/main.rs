//! Mechanism probe only: no claim about arbitrary Rust/library/signal safety.
use open_coroutine_core::scheduler::Scheduler;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

const ITERATIONS: u64 = 100_000_000;
const SAMPLE_MASK: u64 = (1 << 14) - 1;

#[derive(Default)]
struct Observation {
    progress: [AtomicU64; 2],
    checksum: [AtomicU64; 2],
    started_on: [AtomicI32; 2],
    finished_on: [AtomicI32; 2],
    saw_other_running: [AtomicBool; 2],
    done: [AtomicBool; 2],
    last_running: AtomicUsize,
    sampled_dispatch_changes: AtomicUsize,
}

fn thread_id() -> i32 {
    // Read-only Linux thread identity; no context switching or signal handling here.
    unsafe { libc::syscall(libc::SYS_gettid) as i32 }
}

fn compute(index: usize, shared: &Observation) {
    shared.started_on[index].store(thread_id(), Ordering::SeqCst);
    let other = 1 - index;
    let mut value = index as u64 + 1;
    for step in 1..=ITERATIONS {
        value = std::hint::black_box(
            value
                .wrapping_mul(6364136223846793005)
                .wrapping_add(step)
                .rotate_left(7),
        );
        if step & SAMPLE_MASK == 0 {
            shared.progress[index].store(step, Ordering::SeqCst);
            if shared.progress[other].load(Ordering::SeqCst) > 0
                && !shared.done[other].load(Ordering::SeqCst)
            {
                shared.saw_other_running[index].store(true, Ordering::SeqCst);
            }
            if shared.last_running.swap(index + 1, Ordering::SeqCst) != index + 1 {
                shared
                    .sampled_dispatch_changes
                    .fetch_add(1, Ordering::SeqCst);
            }
        }
    }
    shared.checksum[index].store(value, Ordering::SeqCst);
    shared.progress[index].store(ITERATIONS, Ordering::SeqCst);
    shared.done[index].store(true, Ordering::SeqCst);
    shared.finished_on[index].store(thread_id(), Ordering::SeqCst);
}

fn main() -> std::io::Result<()> {
    let worker = thread_id();
    let observations = Arc::new(Observation::default());
    let mut scheduler = Scheduler::default();
    for index in 0..2 {
        let shared = Arc::clone(&observations);
        scheduler.submit_co(
            move |_, _| {
                compute(index, &shared);
                Some(index)
            },
            None,
            None,
        )?;
    }
    let start = Instant::now();
    let results = scheduler.try_schedule()?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let dispatch_changes = observations.sampled_dispatch_changes.load(Ordering::SeqCst);
    let same_worker = (0..2).all(|i| {
        observations.started_on[i].load(Ordering::SeqCst) == worker
            && observations.finished_on[i].load(Ordering::SeqCst) == worker
    });
    let overlap = (0..2).all(|i| observations.saw_other_running[i].load(Ordering::SeqCst));
    let complete = results.len() == 2
        && results.values().all(|r| r.is_ok())
        && (0..2).all(|i| observations.progress[i].load(Ordering::SeqCst) == ITERATIONS);
    let passed = same_worker && overlap && complete && dispatch_changes >= 4;
    println!("{{\"preemptive_feature\":{},\"worker_tid\":{},\"task_worker_count\":1,\"monitor_thread_expected\":{},\"iterations_each\":{},\"elapsed_ms\":{},\"sampled_dispatch_changes\":{},\"same_worker\":{},\"both_observed_overlap\":{},\"complete\":{},\"passed\":{},\"checksums\":[{},{}],\"started_on_tids\":[{},{}],\"finished_on_tids\":[{},{}]}}",
        cfg!(feature = "preemptive"), worker, cfg!(feature = "preemptive"), ITERATIONS,
        elapsed_ms, dispatch_changes, same_worker, overlap, complete, passed,
        observations.checksum[0].load(Ordering::SeqCst), observations.checksum[1].load(Ordering::SeqCst),
        observations.started_on[0].load(Ordering::SeqCst), observations.started_on[1].load(Ordering::SeqCst),
        observations.finished_on[0].load(Ordering::SeqCst), observations.finished_on[1].load(Ordering::SeqCst));
    if !passed {
        return Err(std::io::Error::other(
            "no qualifying same-worker preemption observed",
        ));
    }
    Ok(())
}
