#![forbid(unsafe_code)]
//! Queue/backend comparison only. This is not a Bream process scheduler.
use std::collections::VecDeque;
use std::hint::black_box;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

struct Batch {
    first: usize,
    count: usize,
    published: Instant,
}

struct Queue {
    items: VecDeque<Batch>,
    closed: bool,
    waiting: bool,
}

fn payload(value: usize) -> u64 {
    let mut x = value as u64;
    for _ in 0..32 {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
        x ^= x >> 23;
    }
    black_box(x)
}

fn percentile(sorted: &[u128], percentile: usize) -> u128 {
    sorted[((sorted.len() - 1) * percentile) / 100]
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let policy = args.get(1).expect("policy");
    assert!(["condvar_always", "condvar_coalesced", "park_always", "park_coalesced"].contains(&policy.as_str()));
    let mode = args.get(2).expect("workload");
    let (batches, count, gap) = match mode.as_str() {
        "saturated" => (32768usize, 8usize, 0u64),
        "sparse" => (256, 8, 200),
        "burst" => (256, 128, 200),
        _ => panic!("unknown workload"),
    };
    let park = policy.starts_with("park_");
    let coalesced = policy.ends_with("_coalesced");
    let state = Arc::new((Mutex::new(Queue {
        items: VecDeque::with_capacity(batches),
        closed: false,
        waiting: false,
    }), Condvar::new()));
    let receiver = state.clone();
    // All heap allocation for latency recording is before wait enrollment.
    let worker = thread::spawn(move || {
        let mut received = 0;
        let mut checksum = 0u64;
        let mut waits = 0usize;
        let mut latencies = Vec::with_capacity(batches);
        loop {
            let mut q = receiver.0.lock().unwrap();
            if let Some(batch) = q.items.pop_front() {
                q.waiting = false;
                drop(q);
                latencies.push(batch.published.elapsed().as_nanos());
                assert_eq!(batch.first, received, "lost, duplicated or reordered work");
                for value in batch.first..batch.first + batch.count {
                    checksum = checksum.wrapping_add(payload(value));
                    received += 1;
                }
            } else if q.closed {
                break;
            } else {
                waits += 1;
                q.waiting = true;
                if park {
                    // No callback/logging/other blocking operation in this token interval.
                    drop(q);
                    thread::park();
                } else {
                    drop(receiver.1.wait(q).unwrap());
                }
            }
        }
        (received, checksum, waits, latencies)
    });
    let mut notifications = 0usize;
    let mut issue_lateness = Vec::with_capacity(batches);
    let start = Instant::now();
    for batch in 0..batches {
        let target = start + Duration::from_micros(gap * batch as u64);
        if gap > 0 {
            if let Some(delay) = target.checked_duration_since(Instant::now()) {
                thread::sleep(delay);
            }
        }
        let mut q = state.0.lock().unwrap();
        let published = Instant::now();
        if gap > 0 {
            issue_lateness.push(published.saturating_duration_since(target).as_nanos());
        }
        q.items.push_back(Batch { first: batch * count, count, published });
        let notify = !coalesced || q.waiting;
        q.waiting = false;
        drop(q);
        if notify {
            notifications += 1;
            if park { worker.thread().unpark(); } else { state.1.notify_one(); }
        }
    }
    // This close is ordered after all publication and must wake an idle worker.
    let mut q = state.0.lock().unwrap();
    q.closed = true;
    let notify = !coalesced || q.waiting;
    q.waiting = false;
    drop(q);
    if notify {
        notifications += 1;
        if park { worker.thread().unpark(); } else { state.1.notify_one(); }
    }
    let (received, checksum, waits, mut latencies) = worker.join().unwrap();
    let elapsed = start.elapsed().as_nanos();
    assert_eq!(received, batches * count);
    let expected = (0..received).fold(0u64, |sum, value| sum.wrapping_add(payload(value)));
    assert_eq!(checksum, expected);
    assert!(state.0.lock().unwrap().items.is_empty());
    latencies.sort_unstable();
    issue_lateness.sort_unstable();
    let issue_p99 = if issue_lateness.is_empty() { 0 } else { percentile(&issue_lateness, 99) };
    println!("{{\"policy\":\"{policy}\",\"workload\":\"{mode}\",\"batches\":{batches},\"items\":{received},\"checksum\":{checksum},\"elapsed_ns\":{elapsed},\"notify_calls\":{notifications},\"wait_calls\":{waits},\"queue_delay_p50_ns\":{},\"queue_delay_p99_ns\":{},\"queue_delay_max_ns\":{},\"issue_lateness_p99_ns\":{issue_p99},\"correct\":true}}", percentile(&latencies, 50), percentile(&latencies, 99), latencies[latencies.len()-1]);
}
