use bream_milestone_macros::process;
use bream_milestone_runtime::{ProcessHandle, Runtime, Snapshot};
use std::time::{Duration, Instant};

// Authored computation: owning data, mutation, accumulation and ordinary loops.
// No authored async, yields, cancellation tests, custom callbacks or destructors.
#[process]
fn transform_forever() {
    let mut words = owned_input();
    let mut passes: u64 = 0;
    loop {
        for index in 0..len(words) {
            let value = read(words, index);
            write(words, index, (value * 6364136223846793005 + 17) ^ index);
        }
        passes += 1;
        observe(passes);
    }
}

#[process]
fn transform_and_digest() {
    let mut words = owned_input();
    let mut digest: u64 = 0;
    for index in 0..len(words) {
        let value = read(words, index);
        write(words, index, (value * 6364136223846793005 + 17) ^ index);
        digest += read(words, index) * (index + 1);
        observe(index + 1);
    }
    result(digest);
}

#[process]
fn empty_loop() { loop {} }

#[process]
fn continued_nested_loop() {
    let mut count: u64 = 0;
    loop {
        while true {
            count += 1;
            if count < 100 { continue; }
            break;
        }
        observe(count);
    }
}

#[process]
fn invalid_index() {
    let words = owned_input();
    result(read(words, len(words)));
}

fn input(count: u64) -> Vec<u64> { (0..count).map(|i| i.wrapping_mul(31).wrapping_add(7)).collect() }
fn expected(count: u64) -> u64 {
    input(count).into_iter().enumerate().fold(0u64, |sum, (i, value)| {
        let changed = value.wrapping_mul(6364136223846793005).wrapping_add(17) ^ i as u64;
        sum.wrapping_add(changed.wrapping_mul(i as u64 + 1))
    })
}
fn wait(handle: &ProcessHandle, predicate: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let snapshot = handle.snapshot();
        if predicate(&snapshot) { return snapshot; }
        assert!(Instant::now() < deadline, "watchdog: process condition did not complete");
        std::thread::yield_now();
    }
}
fn stopped(handle: &ProcessHandle, expected_bytes: u64, terminal: u8) -> Snapshot {
    let snapshot = wait(handle, |s| s.cleanup_ns > 0);
    assert_eq!(snapshot.terminal, terminal);
    assert_eq!((snapshot.live_bytes, snapshot.freed_bytes, snapshot.drops), (0, expected_bytes, 1));
    assert!(!snapshot.polling && snapshot.cleanup_ns >= snapshot.stop_ns);
    if terminal == 2 { assert_eq!(snapshot.drops_during_poll, 0); }
    snapshot
}

fn controls(allowance: u32) {
    // Killing a never-polled task must release the context-owned input too.
    let queued_runtime = Runtime::paused(allowance);
    let queued = queued_runtime.spawn(empty_loop(), input(8)).unwrap();
    queued.kill();
    queued_runtime.start();
    let q = stopped(&queued, 64, 2);
    assert_eq!((q.polls, q.progress, q.first_poll_ns), (0, 0, 0));
    queued.kill(); // stale handle changes no other lifetime
    let peer = queued_runtime.spawn(transform_and_digest(), input(64)).unwrap();
    assert_eq!(stopped(&peer, 512, 1).result, expected(64));
    assert_eq!(queued.snapshot(), q);
    queued_runtime.shutdown();

    let runtime = Runtime::new(allowance);
    let empty = runtime.spawn(empty_loop(), input(1)).unwrap();
    wait(&empty, |s| s.polls >= 20);
    empty.kill();
    stopped(&empty, 8, 2);
    let nested = runtime.spawn(continued_nested_loop(), input(1)).unwrap();
    wait(&nested, |s| s.progress >= 100 && s.polls >= 4);
    nested.kill();
    stopped(&nested, 8, 2);
    // Invalid concrete indexing fails this process and leaves its peer usable.
    let bad = runtime.spawn(invalid_index(), input(4)).unwrap();
    stopped(&bad, 32, 3);
    let peer = runtime.spawn(transform_and_digest(), input(128)).unwrap();
    assert_eq!(stopped(&peer, 1024, 1).result, expected(128));
    let shutdown_target = runtime.spawn(transform_forever(), input(128)).unwrap();
    wait(&shutdown_target, |s| s.progress >= 2);
    runtime.shutdown();
    stopped(&shutdown_target, 1024, 2);

    let runtime = Runtime::new(allowance);
    assert!(runtime.spawn(empty_loop(), input(8193)).is_err());
    let zero = runtime.spawn(transform_and_digest(), Vec::new()).unwrap();
    assert_eq!(stopped(&zero, 0, 1).result, 0);
    runtime.shutdown();
    println!("{{\"kind\":\"controls\",\"passed\":true,\"allowance\":{allowance},\"queued_kill\":true,\"empty_loop\":true,\"nested_continue\":true,\"invalid_index_contained\":true,\"shutdown\":true,\"input_limit\":true,\"zero_input\":true}}");
}

fn measurement(allowance: u32) {
    let runtime = Runtime::new(allowance);
    let hog = runtime.spawn(transform_forever(), input(1024)).unwrap();
    wait(&hog, |s| s.progress >= 2 && s.polls >= 4);
    let before = hog.snapshot();
    let peer = runtime.spawn(transform_and_digest(), input(8192)).unwrap();
    // Do not kill until the useful peer finishes: its throughput and wait
    // observations cover coexistence with the still-runnable hog throughout.
    let p = stopped(&peer, 65536, 1);
    assert_eq!((p.progress, p.result), (8192, expected(8192)));
    let alongside = hog.snapshot();
    assert!(alongside.progress > before.progress && alongside.polls > before.polls && alongside.terminal == 0);
    // The host alone asks for kill; no application poll or mailbox does so.
    wait(&hog, |s| s.polling);
    hog.kill();
    let h = stopped(&hog, 8192, 2);
    assert!(h.kill_ns > 0 && h.kill_ns <= h.stop_ns);
    // Exercise the worker after death, then verify all victim observations are
    // unchanged. This detects a resumed/requeued victim after committed stop.
    let survivor = runtime.spawn(transform_and_digest(), input(256)).unwrap();
    assert_eq!(stopped(&survivor, 2048, 1).result, expected(256));
    assert_eq!(hog.snapshot(), h);
    runtime.shutdown();
    println!(concat!("{{\"kind\":\"measurement\",\"passed\":true,\"allowance\":{},",
        "\"application_workers\":1,\"host_control_threads\":1,\"peer_words\":8192,\"peer_digest\":{},",
        "\"peer_submitted_to_first_poll_ns\":{},\"peer_ready_delay_max_ns\":{},\"peer_completion_ns\":{},",
        "\"peer_polls\":{},\"peer_steps\":{},\"peer_poll_max_ns\":{},",
        "\"hog_passes_while_peer_ran\":{},\"hog_polls_before_kill\":{},\"hog_poll_max_ns\":{},",
        "\"kill_requested_ns\":{},\"execution_stopped_ns\":{},\"cleanup_completed_ns\":{},",
        "\"kill_to_stop_ns\":{},\"stop_to_cleanup_ns\":{},\"request_inside_poll_interval\":{},",
        "\"victim_freed_bytes\":{},\"victim_drops\":{},\"victim_drops_during_poll\":{},\"no_post_stop_resume\":true}}"),
        allowance, p.result, p.first_poll_ns-p.submitted_ns, p.ready_delay_max_ns, p.stop_ns-p.submitted_ns,
        p.polls, p.steps, p.poll_max_ns, alongside.progress-before.progress, h.polls, h.poll_max_ns,
        h.kill_ns, h.stop_ns, h.cleanup_ns, h.stop_ns-h.kill_ns, h.cleanup_ns-h.stop_ns, h.kill_during_poll,
        h.freed_bytes, h.drops, h.drops_during_poll);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let allowance = args.get(2).expect("allowance").parse().unwrap();
    match args[1].as_str() {
        "controls" => controls(allowance),
        "measure" => measurement(allowance),
        _ => panic!("expected controls or measure"),
    }
}
