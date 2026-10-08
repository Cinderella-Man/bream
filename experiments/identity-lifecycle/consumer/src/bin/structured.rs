use bream_identity_macros::process_module;
use bream_identity_runtime::{Input, ProcessHandle, Runtime, Snapshot};
use std::time::{Duration, Instant};

#[process_module]
mod service {
    struct State {
        words: Words,
        digest: u64,
        processed: u64,
    }
    enum Request {
        Mix(u64),
        Inspect,
    }
    fn mix(value: u64, rounds: u64) -> u64 {
        let mut current = value;
        for step in 0..rounds {
            current = (current * 6364136223846793005 + 17) ^ step;
        }
        return current;
    }
    fn decode(tag: u64, argument: u64) -> Request {
        if tag == 0 {
            return Request::Mix(argument);
        }
        return Request::Inspect;
    }
    fn apply(mut state: State, command: Request) -> State {
        match command {
            Request::Mix(rounds) => {
                for index in 0..len(state.words) {
                    let value = mix(mix(read(state.words, index), 2), rounds & 7);
                    write(state.words, index, value);
                    state.digest += read(state.words, index) * (index + 1);
                    state.processed += 1;
                    observe(state.processed);
                }
            }
            Request::Inspect => {
                state.digest = len(state.words);
            }
        }
        return state;
    }
    fn run() {
        let mut state = State {
            words: owned_input(),
            digest: 11,
            processed: 0,
        };
        let command = decode(input_tag(), input_argument());
        state = apply(state, command);
        result(state.digest);
        observe(state.processed);
        mark(900);
    }
}

#[process_module]
mod helper_hog {
    struct State {
        words: Words,
        passes: u64,
        digest: u64,
    }
    fn transform(mut state: State) -> State {
        mark(200);
        loop {
            for index in 0..len(state.words) {
                let changed = (read(state.words, index) * 31 + 7) ^ index;
                write(state.words, index, changed);
                state.digest += changed;
            }
            state.passes += 1;
            observe(state.passes);
            result(state.digest);
        }
    }
    fn run() {
        let state = State {
            words: owned_input(),
            passes: 0,
            digest: 0,
        };
        mark(199);
        let finished = transform(state);
        result(finished.digest);
        mark(201);
    }
}

#[process_module]
mod partial_first {
    struct State {
        words: Words,
        seed: u64,
    }
    fn seed(rounds: u64) -> u64 {
        mark(100);
        let mut value: u64 = 3;
        for index in 0..rounds {
            value = value * 31 + index;
            observe(index + 1);
        }
        return value;
    }
    fn run() {
        let state = State {
            words: owned_input(),
            seed: seed(input_argument()),
        };
        mark(101);
        result(state.seed + len(state.words));
    }
}

#[process_module]
mod partial_later {
    struct State {
        words: Words,
        seed: u64,
    }
    fn seed(rounds: u64) -> u64 {
        mark(100);
        let mut value: u64 = 3;
        for index in 0..rounds {
            value = value * 31 + index;
            observe(index + 1);
        }
        return value;
    }
    fn run() {
        // Authored order differs from field declaration order deliberately.
        let state = State {
            seed: seed(input_argument()),
            words: owned_input(),
        };
        mark(101);
        result(state.seed + len(state.words));
    }
}

#[process_module]
mod argument_owner {
    struct State {
        words: Words,
        seed: u64,
    }
    fn seed(rounds: u64) -> u64 {
        mark(700);
        let mut value: u64 = 3;
        for index in 0..rounds {
            value = value * 31 + index;
            observe(index + 1);
        }
        return value;
    }
    fn build(words: Words, value: u64) -> State {
        return State {
            words: words,
            seed: value,
        };
    }
    fn run() {
        let state = build(owned_input(), seed(input_argument()));
        mark(701);
        result(state.seed + len(state.words));
    }
}

#[process_module]
mod nested_state {
    struct Counts {
        total: u64,
        active: bool,
    }
    struct State {
        words: Words,
        counts: Counts,
    }
    fn increment(mut state: State) -> State {
        if state.counts.active {
            state.counts.total += len(state.words);
        }
        return state;
    }
    fn forbidden_path() -> bool {
        let duplicate = owned_input();
        return len(duplicate) > 0;
    }
    fn run() {
        let mut state = State {
            words: owned_input(),
            counts: Counts {
                total: 7,
                active: true,
            },
        };
        let left = false && forbidden_path();
        let right = true || forbidden_path();
        if left || !right {
            result(999);
            return;
        }
        state = increment(state);
        result(state.counts.total);
        mark(800);
    }
}

#[process_module]
mod partial_failure {
    struct State {
        words: Words,
        seed: u64,
    }
    fn fail() -> u64 {
        mark(300);
        let duplicate = owned_input();
        return len(duplicate);
    }
    fn run() {
        let state = State {
            words: owned_input(),
            seed: fail(),
        };
        mark(301);
        result(state.seed);
    }
}

#[process_module]
mod helper_failure {
    struct State {
        words: Words,
        value: u64,
    }
    fn fail(state: State) -> State {
        mark(400);
        let invalid = read(state.words, len(state.words));
        result(invalid);
        return state;
    }
    fn run() {
        let state = State {
            words: owned_input(),
            value: 0,
        };
        let finished = fail(state);
        result(finished.value);
        mark(401);
    }
}

#[process_module]
mod shared_budget {
    fn increment(value: u64) -> u64 {
        return value + 1;
    }
    fn run() {
        let value = increment(increment(1));
        result(value);
    }
}

#[process_module]
mod argument_order {
    fn first() -> u64 {
        mark(11);
        return 1;
    }
    fn second() -> u64 {
        mark(22);
        return 2;
    }
    fn combine(left: u64, right: u64) -> u64 {
        return left * 10 + right;
    }
    fn run() {
        result(combine(first(), second()));
    }
}

#[process_module]
mod empty_helper {
    fn spin() {
        loop {}
    }
    fn run() {
        spin();
        mark(501);
    }
}

#[process_module]
mod nested_helper {
    fn spin() {
        let mut count: u64 = 0;
        loop {
            while true {
                count += 1;
                if count < 100 {
                    continue;
                }
                break;
            }
            observe(count);
        }
    }
    fn run() {
        spin();
        mark(601);
    }
}

fn words(count: u64) -> Vec<u64> {
    (0..count)
        .map(|i| i.wrapping_mul(31).wrapping_add(7))
        .collect()
}
fn input(count: u64, tag: u64, argument: u64) -> Input {
    Input {
        words: words(count),
        tag,
        argument,
    }
}
fn expected(count: u64, rounds: u64) -> u64 {
    fn mix(mut value: u64, rounds: u64) -> u64 {
        for step in 0..rounds {
            value = value.wrapping_mul(6364136223846793005).wrapping_add(17) ^ step;
        }
        value
    }
    words(count)
        .into_iter()
        .enumerate()
        .fold(11u64, |sum, (index, value)| {
            sum.wrapping_add(mix(mix(value, 2), rounds & 7).wrapping_mul(index as u64 + 1))
        })
}
fn seed(rounds: u64) -> u64 {
    (0..rounds).fold(3u64, |value, index| {
        value.wrapping_mul(31).wrapping_add(index)
    })
}
fn wait(handle: &ProcessHandle, predicate: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let s = handle.snapshot();
        if predicate(&s) {
            return s;
        }
        assert!(Instant::now() < deadline, "watchdog condition");
        std::thread::yield_now();
    }
}
fn settled(handle: &ProcessHandle, bytes: u64, reason: u8) -> Snapshot {
    let s = wait(handle, |s| s.cleanup_ns > 0);
    assert_eq!(
        (s.terminal, s.live_bytes, s.freed_bytes, s.drops),
        (reason, 0, bytes, 1)
    );
    assert!(!s.polling && s.cleanup_ns >= s.stop_ns);
    if s.kill_ns != 0 {
        assert!(s.kill_ns <= s.stop_ns);
    }
    if reason == 2 {
        assert_eq!(s.drops_during_poll, 0);
    }
    s
}
fn finite_peer(runtime: &Runtime) -> Snapshot {
    let p = runtime
        .spawn_input(service::program(), input(256, 0, 3))
        .unwrap();
    let s = settled(&p, 2048, 1);
    assert_eq!(
        (s.result, s.progress, s.stage),
        (expected(256, 3), 256, 900)
    );
    s
}
fn controls(allowance: u32) {
    let runtime = Runtime::new(allowance);
    for (tag, rounds) in [(0, 0), (0, 3), (0, 7), (1, 3)] {
        let h = runtime
            .spawn_input(service::program(), input(128, tag, rounds))
            .unwrap();
        let s = settled(&h, 1024, 1);
        assert_eq!(s.result, if tag == 0 { expected(128, rounds) } else { 128 });
        assert_eq!(s.progress, if tag == 0 { 128 } else { 0 });
        assert_eq!(s.stage, 900);
    }
    let budget = runtime.spawn(shared_budget::program(), vec![]).unwrap();
    let b = settled(&budget, 0, 1);
    assert_eq!((b.result, b.steps), (3, 6));
    assert_eq!(b.polls, (6u64 + allowance as u64 - 1) / allowance as u64);
    let ordered = runtime.spawn(argument_order::program(), vec![]).unwrap();
    let o = settled(&ordered, 0, 1);
    assert_eq!((o.result, o.stage), (12, 22));
    for program in [partial_first::program(), partial_later::program()] {
        let h = runtime.spawn_input(program, input(16, 0, 32)).unwrap();
        let s = settled(&h, 128, 1);
        assert_eq!(
            (s.result, s.stage, s.input_taken),
            (seed(32).wrapping_add(16), 101, true)
        );
    }
    let argument = runtime
        .spawn_input(argument_owner::program(), input(16, 0, 32))
        .unwrap();
    let a = settled(&argument, 128, 1);
    assert_eq!((a.result, a.stage), (seed(32).wrapping_add(16), 701));
    let nested = runtime.spawn(nested_state::program(), words(16)).unwrap();
    let n = settled(&nested, 128, 1);
    assert_eq!((n.result, n.stage), (23, 800));
    let mut observations = Vec::new();
    for (name, program, stage, taken) in [
        ("partial_field_owned", partial_first::program(), 100, true),
        (
            "later_field_not_initialized",
            partial_later::program(),
            100,
            false,
        ),
        ("record_owned_by_helper", helper_hog::program(), 200, true),
        (
            "owning_argument_before_suspending_argument",
            argument_owner::program(),
            700,
            true,
        ),
    ] {
        let h = runtime
            .spawn_input(program, input(128, 0, 1_000_000_000))
            .unwrap();
        let before = wait(&h, |s| s.stage == stage && s.progress >= 2 && s.polls >= 4);
        assert_eq!(before.input_taken, taken);
        finite_peer(&runtime);
        let after_peer = h.snapshot();
        assert!(
            after_peer.polls > before.polls
                && after_peer.progress > before.progress
                && after_peer.terminal == 0
        );
        h.kill();
        let stopped = settled(&h, 1024, 2);
        assert_eq!((stopped.stage, stopped.input_taken), (stage, taken));
        finite_peer(&runtime);
        assert_eq!(h.snapshot(), stopped);
        observations.push(format!("{{\"scenario\":\"{name}\",\"stage\":{stage},\"input_taken\":{taken},\"polls\":{},\"steps\":{},\"frame_bytes\":{},\"kill_to_stop_ns\":{},\"stop_to_cleanup_ns\":{},\"drops\":{},\"freed_bytes\":{},\"no_post_stop_resume\":true}}",
            stopped.polls, stopped.steps, stopped.frame_bytes, stopped.stop_ns-stopped.kill_ns,
            stopped.cleanup_ns-stopped.stop_ns, stopped.drops, stopped.freed_bytes));
    }
    for (program, stage) in [
        (partial_failure::program(), 300),
        (helper_failure::program(), 400),
    ] {
        let h = runtime.spawn_input(program, input(16, 0, 0)).unwrap();
        let s = settled(&h, 128, 3);
        assert_eq!(
            (s.stage, s.input_taken, s.drops_during_poll),
            (stage, true, 1)
        );
        finite_peer(&runtime);
    }
    for program in [empty_helper::program(), nested_helper::program()] {
        let h = runtime.spawn(program, words(8)).unwrap();
        wait(&h, |s| s.polls >= 20);
        h.kill();
        let s = settled(&h, 64, 2);
        assert_eq!(s.stage, 0);
    }
    let shutdown = runtime
        .spawn_input(helper_hog::program(), input(16, 0, 0))
        .unwrap();
    wait(&shutdown, |s| s.stage == 200 && s.polls >= 4);
    runtime.shutdown();
    settled(&shutdown, 128, 2);
    println!("{{\"kind\":\"controls\",\"passed\":true,\"allowance\":{allowance},\"shared_budget_steps\":{},\"shared_budget_polls\":{},\"enum_variants\":2,\"helper_argument_order\":true,\"partial_failure\":true,\"helper_failure\":true,\"empty_helper\":true,\"nested_continue\":true,\"shutdown\":true,\"kill_scenarios\":[{}]}}", b.steps, b.polls, observations.join(","));
}
fn measurement(allowance: u32) {
    let runtime = Runtime::new(allowance);
    let hog = runtime
        .spawn_input(helper_hog::program(), input(1024, 0, 0))
        .unwrap();
    let before = wait(&hog, |s| s.stage == 200 && s.progress >= 2 && s.polls >= 4);
    let peer = runtime
        .spawn_input(service::program(), input(8192, 0, 3))
        .unwrap();
    let p = settled(&peer, 65536, 1);
    assert_eq!(
        (p.result, p.progress, p.stage),
        (expected(8192, 3), 8192, 900)
    );
    let alongside = hog.snapshot();
    assert!(
        alongside.progress > before.progress
            && alongside.polls > before.polls
            && alongside.terminal == 0
    );
    hog.kill();
    let h = settled(&hog, 8192, 2);
    assert_eq!(h.stage, 200);
    finite_peer(&runtime);
    assert_eq!(hog.snapshot(), h);
    runtime.shutdown();
    println!(concat!("{{\"kind\":\"measurement\",\"passed\":true,\"allowance\":{},\"peer_words\":8192,",
        "\"peer_digest\":{},\"peer_steps\":{},\"peer_polls\":{},\"peer_frame_bytes\":{},\"hog_frame_bytes\":{},",
        "\"peer_completion_ns\":{},\"peer_first_poll_ns\":{},\"peer_ready_delay_max_ns\":{},",
        "\"hog_passes_during_peer\":{},\"kill_to_stop_ns\":{},\"stop_to_cleanup_ns\":{},",
        "\"kill_requested_ns\":{},\"execution_stopped_ns\":{},\"cleanup_completed_ns\":{},",
        "\"victim_freed_bytes\":{},\"victim_drops\":{},\"victim_drops_during_poll\":{},\"no_post_stop_resume\":true}}"),
        allowance, p.result, p.steps, p.polls, p.frame_bytes, h.frame_bytes,
        p.stop_ns-p.submitted_ns, p.first_poll_ns-p.submitted_ns, p.ready_delay_max_ns,
        alongside.progress-before.progress, h.stop_ns-h.kill_ns, h.cleanup_ns-h.stop_ns,
        h.kill_ns, h.stop_ns, h.cleanup_ns, h.freed_bytes, h.drops, h.drops_during_poll);
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let allowance = args[2].parse().unwrap();
    match args[1].as_str() {
        "controls" => controls(allowance),
        "measure" => measurement(allowance),
        _ => panic!("controls or measure"),
    }
}
