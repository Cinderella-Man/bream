use bream_identity_macros::process_module;
use bream_identity_runtime::{
    Input, KillReceipt, LookupError, ProcessHandle, ProcessId, Runtime, Snapshot, SpawnError,
    SpawnErrorKind,
};
use std::collections::HashSet;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

#[process_module]
mod finite {
    struct State {
        words: Words,
        sum: u64,
    }
    fn transform(mut state: State, rounds: u64) -> State {
        for round in 0..rounds {
            for index in 0..len(state.words) {
                let next = (read(state.words, index) * 31 + 7) ^ round;
                write(state.words, index, next);
                state.sum += next;
            }
        }
        return state;
    }
    fn run() {
        let state = State {
            words: owned_input(),
            sum: 0,
        };
        let done = transform(state, input_argument());
        result(done.sum);
        mark(900);
    }
}
#[process_module]
mod hog {
    struct State {
        words: Words,
        passes: u64,
    }
    fn run() {
        let mut state = State {
            words: owned_input(),
            passes: 0,
        };
        loop {
            for index in 0..len(state.words) {
                write(state.words, index, read(state.words, index) + 1);
            }
            state.passes += 1;
            observe(state.passes);
        }
    }
}
#[process_module]
mod faulty {
    fn run() {
        let words = owned_input();
        result(read(words, len(words)));
    }
}

fn words(n: usize) -> Vec<u64> {
    (0..n as u64).collect()
}
fn input(n: usize, rounds: u64) -> Input {
    Input {
        words: words(n),
        tag: 0,
        argument: rounds,
    }
}
fn expected(n: usize, rounds: u64) -> u64 {
    let mut data = words(n);
    let mut result = 0_u64;
    for round in 0..rounds {
        for value in &mut data {
            *value = value.wrapping_mul(31).wrapping_add(7) ^ round;
            result = result.wrapping_add(*value);
        }
    }
    result
}
fn wait(handle: &ProcessHandle, condition: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let s = handle.snapshot();
        if condition(&s) {
            return s;
        }
        assert!(Instant::now() < deadline, "process watchdog");
        std::thread::yield_now();
    }
}
fn finished(handle: &ProcessHandle, bytes: u64) -> Snapshot {
    let s = wait(handle, |s| s.cleanup_ns > 0);
    assert!(s.terminal > 0 && !s.polling && s.stop_ns > 0 && s.cleanup_ns >= s.stop_ns);
    assert_eq!((s.live_bytes, s.freed_bytes, s.drops), (0, bytes, 1));
    s
}
fn rejected(error: SpawnError, kind: SpawnErrorKind, bytes: u64) {
    assert_eq!(error.kind, kind);
    let s = error.cleanup.unwrap();
    assert_eq!(
        (s.polls, s.live_bytes, s.freed_bytes, s.drops, s.terminal),
        (0, 0, bytes, 1, 4)
    );
    assert!(s.cleanup_ns >= s.stop_ns && s.stop_ns > 0);
}

fn controls(allowance: u32) {
    let runtime = Runtime::paused_with_capacity(allowance, 1);
    let client = runtime.client();
    let old = runtime.spawn(hog::program(), words(8)).unwrap();
    let old_id = old.id();
    assert!(client.is_live(old_id));
    assert_eq!(client.lookup(old_id).unwrap().id(), old_id);
    rejected(
        runtime.spawn(hog::program(), words(8)).err().unwrap(),
        SpawnErrorKind::Capacity,
        64,
    );
    let foreign = Runtime::paused_with_capacity(allowance, 1);
    let other = foreign.spawn(hog::program(), words(8)).unwrap();
    assert_eq!(
        (old_id.slot(), old_id.generation()),
        (other.id().slot(), other.id().generation())
    );
    assert_ne!(old_id.runtime(), other.id().runtime());
    assert_eq!(
        foreign.client().kill(old_id),
        Err(LookupError::WrongRuntime)
    );
    assert!(!foreign.client().is_live(old_id));
    assert_eq!(old.kill(), KillReceipt::Accepted);
    assert_eq!(old.kill(), KillReceipt::AlreadyRequested);
    runtime.start();
    let old_done = finished(&old, 64);
    assert_eq!((old_done.terminal, old_done.polls), (2, 0));
    let new = runtime.spawn(hog::program(), words(8)).unwrap();
    assert_eq!(new.id().slot(), old_id.slot());
    assert_eq!(new.id().generation(), old_id.generation() + 1);
    assert_eq!(client.kill(old_id), Err(LookupError::NotLive));
    assert_eq!(old.kill(), KillReceipt::TooLate);
    assert_eq!(old.snapshot(), old_done);
    wait(&new, |s| s.progress >= 2);
    assert_eq!(client.kill(new.id()), Ok(KillReceipt::Accepted));
    assert_eq!(finished(&new, 64).terminal, 2);
    assert_eq!(client.live_count(), 0);
    foreign.shutdown();
    assert_eq!(finished(&other, 64).polls, 0);

    let detached = runtime.spawn(hog::program(), words(8)).unwrap();
    let detached_id = detached.id();
    drop(detached);
    let looked_up = client.lookup(detached_id).unwrap();
    wait(&looked_up, |s| s.progress >= 2);
    assert_eq!(client.kill(detached_id), Ok(KillReceipt::Accepted));
    assert_eq!(finished(&looked_up, 64).terminal, 2);
    let mut retained = vec![old, new, looked_up];
    for _ in 0..128 {
        let next = runtime.spawn_input(finite::program(), input(8, 2)).unwrap();
        let done = finished(&next, 64);
        assert_eq!((done.terminal, done.result), (1, expected(8, 2)));
        assert_eq!(client.live_count(), 0);
        for previous in &retained {
            assert!(!client.is_live(previous.id()));
            assert_eq!(client.kill(previous.id()), Err(LookupError::NotLive));
            assert_eq!(previous.kill(), KillReceipt::TooLate);
        }
        assert_eq!(next.id().slot(), old_id.slot());
        assert_eq!(
            next.id().generation(),
            retained.last().unwrap().id().generation() + 1
        );
        retained.push(next);
    }
    let bad = runtime.spawn(faulty::program(), words(8)).unwrap();
    assert_eq!(finished(&bad, 64).terminal, 3);
    let survivor = runtime.spawn_input(finite::program(), input(8, 1)).unwrap();
    assert_eq!(finished(&survivor, 64).result, expected(8, 1));
    runtime.shutdown();
    rejected(
        client.spawn(hog::program(), words(8)).err().unwrap(),
        SpawnErrorKind::Closed,
        64,
    );

    for running in [false, true] {
        let rt = if running {
            Runtime::with_capacity(allowance, 4)
        } else {
            Runtime::paused_with_capacity(allowance, 4)
        };
        let client = rt.client();
        let handles: Vec<_> = (0..4)
            .map(|_| rt.spawn(hog::program(), words(8)).unwrap())
            .collect();
        if running {
            for h in &handles {
                wait(h, |s| s.progress > 1);
            }
        }
        drop(rt);
        for h in &handles {
            let done = finished(h, 64);
            assert_eq!(done.terminal, 2);
            assert_eq!(done.drops_during_poll, 0);
            if !running {
                assert_eq!(done.polls, 0);
            }
            assert!(!client.is_live(h.id()));
        }
        assert_eq!(client.live_count(), 0);
        rejected(
            client.spawn(hog::program(), words(8)).err().unwrap(),
            SpawnErrorKind::Closed,
            64,
        );
    }

    let rt = Runtime::with_capacity(allowance, 1);
    let client = rt.client();
    let mut won = 0;
    let mut late = 0;
    for sample in 0..200 {
        let (size, rounds) = if sample % 2 == 0 { (8, 2) } else { (256, 16) };
        let h = rt
            .spawn_input(finite::program(), input(size, rounds))
            .unwrap();
        let killer = h.clone();
        let receipt = std::thread::spawn(move || killer.kill()).join().unwrap();
        let done = finished(&h, size as u64 * 8);
        match receipt {
            KillReceipt::Accepted => {
                won += 1;
                assert_eq!(done.terminal, 2);
            }
            KillReceipt::TooLate => {
                late += 1;
                assert_eq!((done.terminal, done.result), (1, expected(size, rounds)));
            }
            KillReceipt::AlreadyRequested => panic!("only one kill issuer"),
        }
        assert_eq!(client.live_count(), 0);
        assert_eq!(h.kill(), KillReceipt::TooLate);
        assert_eq!(h.snapshot(), done);
    }
    rt.shutdown();

    let rt = Runtime::paused_with_capacity(allowance, 4);
    let barrier = Arc::new(Barrier::new(9));
    let publishers: Vec<_> = (0..8)
        .map(|_| {
            let client = rt.client();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                client.spawn(hog::program(), words(8))
            })
        })
        .collect();
    barrier.wait();
    let mut admitted = Vec::new();
    let mut full = 0;
    for thread in publishers {
        match thread.join().unwrap() {
            Ok(h) => admitted.push(h),
            Err(e) => {
                rejected(e, SpawnErrorKind::Capacity, 64);
                full += 1;
            }
        }
    }
    assert_eq!((admitted.len(), full), (4, 4));
    assert_eq!(
        admitted
            .iter()
            .map(|h| h.id())
            .collect::<HashSet<_>>()
            .len(),
        4
    );
    rt.shutdown();
    for h in admitted {
        assert_eq!(finished(&h, 64).terminal, 2);
    }

    let rt = Runtime::paused_with_capacity(allowance, 33);
    let client = rt.client();
    let anchor = rt.spawn(hog::program(), words(8)).unwrap();
    let barrier = Arc::new(Barrier::new(5));
    let publishers: Vec<_> = (0..4)
        .map(|_| {
            let client = client.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                (0..8)
                    .map(|_| client.spawn(hog::program(), words(8)))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    barrier.wait();
    rt.shutdown();
    let mut accepted = 1;
    let mut closed = 0;
    assert_eq!(finished(&anchor, 64).terminal, 2);
    for thread in publishers {
        for result in thread.join().unwrap() {
            match result {
                Ok(h) => {
                    accepted += 1;
                    assert_eq!(finished(&h, 64).terminal, 2);
                }
                Err(e) => {
                    closed += 1;
                    rejected(e, SpawnErrorKind::Closed, 64);
                }
            }
        }
    }
    assert_eq!(accepted + closed, 33);
    assert_eq!(client.live_count(), 0);
    rejected(
        client.spawn(hog::program(), words(8)).err().unwrap(),
        SpawnErrorKind::Closed,
        64,
    );
    println!("{{\"kind\":\"identity_controls\",\"passed\":true,\"allowance\":{allowance},\"reuse_cycles\":128,\"kill_races\":200,\"kill_accepted\":{won},\"kill_too_late\":{late},\"capacity_accepted\":4,\"capacity_rejected\":4,\"shutdown_accepted\":{accepted},\"shutdown_rejected\":{closed},\"live_after_shutdown\":0,\"stale_handles_isolated\":true,\"cross_runtime_rejected\":true}}");
}

fn measure(allowance: u32) {
    let rt = Runtime::with_capacity(allowance, 2);
    let client = rt.client();
    let hog = rt.spawn(hog::program(), words(128)).unwrap();
    wait(&hog, |s| s.progress >= 2);
    let initial = hog.snapshot().progress;
    let start = Instant::now();
    let mut first_max = 0;
    let mut ready_max = 0;
    let mut digest = 0_u64;
    let mut last_id: Option<ProcessId> = None;
    for _ in 0..128 {
        let h = rt.spawn_input(finite::program(), input(64, 2)).unwrap();
        let done = finished(&h, 512);
        assert_eq!((done.terminal, done.result), (1, expected(64, 2)));
        first_max = first_max.max(done.first_poll_ns - done.submitted_ns);
        ready_max = ready_max.max(done.ready_delay_max_ns);
        digest = digest.wrapping_add(done.result);
        if let Some(old) = last_id {
            assert_eq!(h.id().slot(), old.slot());
            assert_eq!(h.id().generation(), old.generation() + 1);
            assert_eq!(client.kill(old), Err(LookupError::NotLive));
        }
        assert!(!client.is_live(h.id()));
        last_id = Some(h.id());
    }
    let elapsed = start.elapsed().as_nanos();
    let progress = hog.snapshot().progress - initial;
    assert!(progress > 0);
    assert_eq!(client.kill(hog.id()), Ok(KillReceipt::Accepted));
    let stopped = finished(&hog, 1024);
    assert_eq!(stopped.terminal, 2);
    assert_eq!(client.live_count(), 0);
    let survivor = rt.spawn_input(finite::program(), input(8, 1)).unwrap();
    assert_eq!(finished(&survivor, 64).result, expected(8, 1));
    assert_eq!(hog.snapshot(), stopped);
    println!("{{\"kind\":\"churn\",\"passed\":true,\"allowance\":{allowance},\"processes\":128,\"digest\":{digest},\"completion_ns\":{elapsed},\"peer_first_poll_max_ns\":{first_max},\"peer_ready_delay_max_ns\":{ready_max},\"hog_passes\":{progress},\"kill_to_stop_ns\":{},\"stop_to_cleanup_ns\":{},\"victim_drops\":{},\"victim_freed_bytes\":{},\"no_post_stop_resume\":true,\"id_bytes\":{},\"handle_bytes\":{}}}", stopped.stop_ns-stopped.kill_ns, stopped.cleanup_ns-stopped.stop_ns, stopped.drops, stopped.freed_bytes, std::mem::size_of::<ProcessId>(), std::mem::size_of::<ProcessHandle>());
    rt.shutdown();
}

fn lookup_cost(mode: &str, state: &str) {
    let rt = Runtime::paused_with_capacity(32, 1);
    let client = rt.client();
    let h = rt.spawn(hog::program(), words(8)).unwrap();
    let id = h.id();
    let alive = state == "live";
    if !alive {
        rt.shutdown();
    }
    let iterations = 1_000_000_u64;
    let start = Instant::now();
    let mut hits = 0_u64;
    match mode {
        "retained" => {
            for _ in 0..iterations {
                hits +=
                    std::hint::black_box(client.lookup(std::hint::black_box(id)).is_ok()) as u64;
            }
        }
        "borrowed" => {
            for _ in 0..iterations {
                hits += std::hint::black_box(client.is_live(std::hint::black_box(id))) as u64;
            }
        }
        _ => panic!("unknown lookup mode"),
    }
    let elapsed = start.elapsed().as_nanos();
    assert_eq!(hits, if alive { iterations } else { 0 });
    println!("{{\"kind\":\"lookup\",\"passed\":true,\"mode\":\"{mode}\",\"state\":\"{state}\",\"iterations\":{iterations},\"hits\":{hits},\"elapsed_ns\":{elapsed}}}");
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("controls") => controls(args[2].parse().unwrap()),
        Some("measure") => measure(args[2].parse().unwrap()),
        Some("lookup") => lookup_cost(&args[2], &args[3]),
        _ => panic!("expected controls ALLOWANCE, measure ALLOWANCE, or lookup retained|borrowed live|stale"),
    }
}
