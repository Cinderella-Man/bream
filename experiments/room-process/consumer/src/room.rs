use super::*;
use bream_room_runtime::RuntimeClient;

#[process_module]
mod application {
    struct Member {
        child: Spawned,
        joined: bool,
        alive: bool,
        left: bool,
        fault: bool,
    }
    struct Counters {
        joins: u64,
        duplicates: u64,
        delivered: u64,
        leaves: u64,
        failures: u64,
        retired: u64,
        errors: u64,
        acks: u64,
    }
    struct State {
        first: Member,
        second: Member,
        counts: Counters,
    }
    struct Summary {
        mask: u64,
        joins: u64,
        duplicates: u64,
        delivered: u64,
        leaves: u64,
        failures: u64,
        retired: u64,
        errors: u64,
    }
    enum Action {
        Report,
        First,
        Second,
        Discard,
        Finish,
    }
    fn report(code: u64, mut words: Words, summary: Summary) {
        write(words, 0, code);
        write(words, 1, summary.mask);
        write(words, 2, summary.joins);
        write(words, 3, summary.duplicates);
        write(words, 4, summary.delivered);
        write(words, 5, summary.leaves);
        write(words, 6, summary.failures);
        write(words, 7, summary.retired);
        write(words, 8, summary.errors);
        send(input_target(), 300, words);
    }
    fn session() {
        let room_pid = input_target();
        let room_monitor = monitor(room_pid);
        let slot = input_argument();
        send(room_pid, 10 + slot, owned_input());
        while true {
            match receive_event() {
                Event::Message(message) => {
                    let tag = message_tag(message);
                    let mut words = message_words(message);
                    if tag == 100 {
                        send(room_pid, 20 + slot, words);
                    } else if tag == 201 {
                        send(room_pid, 10 + slot, words);
                    } else if tag == 202 {
                        write(words, 10, read(words, 10) + slot * 100);
                        send(room_pid, 50 + slot, words);
                    } else if tag == 203 {
                        send(room_pid, 60 + slot, words);
                        result(1);
                        return;
                    } else if tag == 204 {
                        send(room_pid, 70 + slot, words);
                        // A contained application failure after its report.
                        let unavailable = owned_input();
                        result(read(unavailable, 0));
                    } else {
                        result(999);
                    }
                }
                Event::Down(room_down) => {
                    if down_ref(room_down) == room_monitor && down_target(room_down) == room_pid {
                        result(900 + down_reason(room_down));
                        return;
                    }
                    result(999);
                }
                Event::Exit(unexpected_exit) => {
                    result(999);
                }
            }
        }
    }
    fn run() {
        let resident = owned_input();
        let first_input = message_words(receive(1));
        let first = spawn_monitor(session, first_input, 0, 1);
        let second_input = message_words(receive(2));
        let second = spawn_monitor(session, second_input, 0, 2);
        let mut state = State {
            first: Member {
                child: first,
                joined: false,
                alive: true,
                left: false,
                fault: false,
            },
            second: Member {
                child: second,
                joined: false,
                alive: true,
                left: false,
                fault: false,
            },
            counts: Counters {
                joins: 0,
                duplicates: 0,
                delivered: 0,
                leaves: 0,
                failures: 0,
                retired: 0,
                errors: 0,
                acks: 0,
            },
        };
        while true {
            match receive_event() {
                Event::Message(message) => {
                    let tag = message_tag(message);
                    let words = message_words(message);
                    let slot = read(words, 0);
                    let mut action = Action::Discard;
                    let mut command: u64 = 0;
                    let mut code: u64 = 0;
                    if tag == 11 || tag == 12 {
                        if tag == 11 {
                            if state.first.joined {
                                state.counts.duplicates += 1;
                            } else {
                                state.first.joined = true;
                                state.counts.joins += 1;
                            }
                            action = Action::First;
                        } else {
                            if state.second.joined {
                                state.counts.duplicates += 1;
                            } else {
                                state.second.joined = true;
                                state.counts.joins += 1;
                            }
                            action = Action::Second;
                        }
                        command = 100;
                    } else if tag == 21 || tag == 22 {
                        state.counts.acks += 1;
                        if state.counts.acks == 2 {
                            mark(103);
                        }
                        if read(words, 9) != 0 {
                            code = 2;
                            action = Action::Report;
                        }
                    } else if tag == 51 || tag == 52 {
                        state.counts.delivered += 1;
                        code = 7;
                        action = Action::Report;
                    } else if tag == 61 || tag == 62 {
                        if tag == 61 {
                            if state.first.joined {
                                state.counts.leaves += 1;
                            }
                            state.first.joined = false;
                            state.first.left = true;
                        } else {
                            if state.second.joined {
                                state.counts.leaves += 1;
                            }
                            state.second.joined = false;
                            state.second.left = true;
                        }
                        code = 10;
                        action = Action::Report;
                    } else if tag == 71 || tag == 72 {
                        if tag == 71 {
                            state.first.fault = true;
                        } else {
                            state.second.fault = true;
                        }
                        code = 8;
                        action = Action::Report;
                    } else if tag >= 101 && tag <= 104 {
                        let mut available = false;
                        if slot == 1 {
                            available = state.first.joined && state.first.alive;
                        } else if slot == 2 {
                            available = state.second.joined && state.second.alive;
                        }
                        if slot != 1 && slot != 2 {
                            code = 5;
                            action = Action::Report;
                        } else if !available {
                            code = 4;
                            if tag == 103 {
                                code = 3;
                            }
                            action = Action::Report;
                        } else {
                            command = tag + 100;
                            if slot == 1 {
                                action = Action::First;
                            } else {
                                action = Action::Second;
                            }
                        }
                    } else if tag == 105 {
                        code = 1;
                        action = Action::Report;
                    } else if tag == 106 {
                        mark(200);
                        match receive_for(909, 5) {
                            ReceiveResult::Timeout => {
                                code = 6;
                            }
                            ReceiveResult::Message(reply) => {
                                code = 11;
                            }
                        }
                        action = Action::Report;
                    } else if tag == 107 {
                        if state.counts.retired == 2 {
                            code = 9;
                            action = Action::Finish;
                        } else {
                            code = 12;
                            action = Action::Report;
                        }
                    } else {
                        state.counts.errors += 1;
                    }
                    let mut mask: u64 = 0;
                    if state.first.joined {
                        mask += 1;
                    }
                    if state.second.joined {
                        mask += 2;
                    }
                    let summary = Summary {
                        mask: mask,
                        joins: state.counts.joins,
                        duplicates: state.counts.duplicates,
                        delivered: state.counts.delivered,
                        leaves: state.counts.leaves,
                        failures: state.counts.failures,
                        retired: state.counts.retired,
                        errors: state.counts.errors,
                    };
                    match action {
                        Action::Report => {
                            report(code, words, summary);
                        }
                        Action::First => {
                            send(spawned_pid(state.first.child), command, words);
                        }
                        Action::Second => {
                            send(spawned_pid(state.second.child), command, words);
                        }
                        Action::Discard => {}
                        Action::Finish => {
                            report(code, words, summary);
                            result(read(resident, 0));
                            return;
                        }
                    }
                }
                Event::Down(notification) => {
                    let dead = down_target(notification);
                    let reference = down_ref(notification);
                    let reason = down_reason(notification);
                    if dead == spawned_pid(state.first.child)
                        && reference == spawned_ref(state.first.child)
                        && state.first.alive
                    {
                        if reason == 1 && !state.first.left {
                            state.counts.errors += 1;
                        }
                        if reason == 3 && !state.first.fault {
                            state.counts.errors += 1;
                        }
                        if reason != 1 {
                            state.counts.failures += 1;
                        }
                        state.first.joined = false;
                        state.first.alive = false;
                        state.counts.retired += 1;
                    } else if dead == spawned_pid(state.second.child)
                        && reference == spawned_ref(state.second.child)
                        && state.second.alive
                    {
                        if reason == 1 && !state.second.left {
                            state.counts.errors += 1;
                        }
                        if reason == 3 && !state.second.fault {
                            state.counts.errors += 1;
                        }
                        if reason != 1 {
                            state.counts.failures += 1;
                        }
                        state.second.joined = false;
                        state.second.alive = false;
                        state.counts.retired += 1;
                    } else {
                        state.counts.errors += 1;
                    }
                    observe(state.counts.retired);
                    mark(300 + state.counts.retired);
                }
                Event::Exit(unexpected) => {
                    state.counts.errors += 1;
                }
            }
        }
    }
}
#[process_module]
mod collector {
    fn run() {
        let mut digest: u64 = 0;
        let mut count: u64 = 0;
        while true {
            let message = receive(300);
            let words = message_words(message);
            for index in 0..11 {
                digest = digest * 131 + read(words, index);
            }
            count += 1;
            result(digest);
            mark(read(words, 9));
            observe(count);
        }
    }
}
#[process_module]
mod computation {
    fn run() {
        let held = owned_input();
        let mut n: u64 = read(held, 0);
        while true {
            n += 1;
            observe(n);
        }
    }
}
#[process_module]
mod computation_observer {
    fn run() {
        let reference = monitor(input_target());
        mark(1);
        let notification = receive_down(reference);
        result(down_reason(notification));
    }
}

struct Fixture {
    runtime: Runtime,
    client: RuntimeClient,
    room: ProcessHandle,
    collector: ProcessHandle,
    hog: ProcessHandle,
    watcher: ProcessHandle,
    receipts: Vec<MessageReceipt>,
    expected: Vec<Vec<u64>>,
    observations: Vec<[u64; 3]>,
    digest: u64,
}
impl Fixture {
    fn new(allowance: u32) -> Self {
        let runtime = Runtime::paused(allowance);
        let client = runtime.client();
        let collector = runtime.spawn(collector::program(), vec![]).unwrap();
        let room = runtime
            .spawn_to(
                application::program(),
                Input {
                    words: vec![7],
                    tag: 0,
                    argument: 0,
                },
                collector.id(),
            )
            .unwrap();
        let hog = runtime.spawn(computation::program(), vec![0; 8]).unwrap();
        let watcher = runtime
            .spawn_to(
                computation_observer::program(),
                Input {
                    words: vec![],
                    tag: 0,
                    argument: 0,
                },
                hog.id(),
            )
            .unwrap();
        let mut receipts = Vec::new();
        for tag in [1, 2] {
            let (message, receipt) = packet(tag, vec![0; 11]);
            client.send(room.id(), message);
            receipts.push(receipt);
        }
        runtime.start();
        wait(&room, |s| s.stage == 103 && s.waiting);
        wait(&watcher, |s| s.stage == 1 && s.waiting);
        wait(&hog, |s| s.progress > 0);
        assert_eq!(
            (
                client.live_count(),
                client.active_monitors(),
                client.active_links()
            ),
            (6, 5, 0)
        );
        released(&receipts, 88);
        Self {
            runtime,
            client,
            room,
            collector,
            hog,
            watcher,
            receipts,
            expected: Vec::new(),
            observations: Vec::new(),
            digest: 0,
        }
    }
    fn send(&mut self, tag: u64, slot: u64, sequence: u64, value: u64) {
        let mut words = vec![0; 11];
        words[0] = slot;
        words[9] = sequence;
        words[10] = value;
        let (message, receipt) = packet(tag, words);
        self.client.send(self.room.id(), message);
        self.receipts.push(receipt);
    }
    fn expect(&mut self, row: Vec<u64>) {
        assert_eq!(row.len(), 11);
        for n in &row {
            self.digest = self.digest.wrapping_mul(131).wrapping_add(*n);
        }
        self.expected.push(row);
    }
    fn check_transcript(&mut self) -> Snapshot {
        let count = self.expected.len() as u64;
        let snapshot = wait(&self.collector, |s| s.progress >= count && s.waiting);
        assert_eq!((snapshot.progress, snapshot.result), (count, self.digest));
        assert_eq!(snapshot.stage, self.expected.last().unwrap()[9]);
        assert_eq!(self.room.snapshot().terminal, 0);
        self.observations
            .push([snapshot.progress, snapshot.result, snapshot.stage]);
        snapshot
    }
    fn peer_progress(&self, before: u64) {
        wait(&self.hog, |s| s.progress > before);
        assert_eq!(self.hog.snapshot().terminal, 0);
    }
    fn finish(self, room_terminal: u8) {
        let room = done(&self.room, room_terminal);
        if room_terminal == 1 {
            assert_eq!(room.result, 7);
        }
        assert_eq!((room.drops, room.live_bytes, room.freed_bytes), (1, 0, 8));
        assert_eq!(self.hog.kill(), KillReceipt::Accepted);
        let stopped = done(&self.hog, 2);
        assert_eq!(
            (stopped.drops, stopped.live_bytes, stopped.freed_bytes),
            (1, 0, 64)
        );
        let watcher = done(&self.watcher, 1);
        assert_eq!(
            (
                watcher.result,
                watcher.drops,
                watcher.live_bytes,
                watcher.freed_bytes
            ),
            (2, 1, 0, 0)
        );
        self.runtime.shutdown();
        let collector = done(&self.collector, 2);
        assert_eq!(
            (collector.drops, collector.live_bytes, collector.freed_bytes),
            (1, 0, 0)
        );
        assert_eq!(
            (
                self.client.live_count(),
                self.client.active_monitors(),
                self.client.active_links()
            ),
            (0, 0, 0)
        );
        assert_eq!(self.client.clock_snapshot().pending, 0);
        released(&self.receipts, 88);
        frozen_application(&room, &self.room.snapshot());
    }
}
fn row(
    code: u64,
    mask: u64,
    duplicates: u64,
    delivered: u64,
    leaves: u64,
    failures: u64,
    retired: u64,
    sequence: u64,
    value: u64,
) -> Vec<u64> {
    vec![
        code, mask, 2, duplicates, delivered, leaves, failures, retired, 0, sequence, value,
    ]
}
pub(super) fn run(allowance: u32) {
    assert!([1, 4, 32].contains(&allowance));
    let mut f = Fixture::new(allowance);
    let before = f.hog.snapshot().progress;
    f.send(105, 0, 1, 0);
    f.expect(row(1, 3, 0, 0, 0, 0, 0, 1, 0));
    f.check_transcript();
    f.send(101, 1, 2, 0);
    f.expect(row(2, 3, 1, 0, 0, 0, 0, 2, 0));
    f.check_transcript();
    assert_eq!(f.client.active_monitors(), 5); // duplicate join adds no monitor
    f.send(102, 1, 3, 42);
    f.expect(row(7, 3, 1, 1, 0, 0, 0, 3, 142));
    f.check_transcript();
    f.send(102, 9, 4, 7);
    f.expect(row(5, 3, 1, 1, 0, 0, 0, 4, 7));
    f.check_transcript();
    f.send(104, 1, 5, 0);
    f.expect(row(8, 3, 1, 1, 0, 0, 0, 5, 0));
    f.check_transcript();
    wait(&f.room, |s| s.progress == 1 && s.waiting);
    f.send(105, 0, 6, 0);
    f.expect(row(1, 2, 1, 1, 0, 1, 1, 6, 0));
    f.check_transcript();
    f.send(102, 1, 7, 9);
    f.expect(row(4, 2, 1, 1, 0, 1, 1, 7, 9));
    f.check_transcript();
    // Survivor handles another request while the independent computation runs.
    f.send(102, 2, 8, 43);
    f.expect(row(7, 2, 1, 2, 0, 1, 1, 8, 243));
    f.check_transcript();
    f.peer_progress(before);
    // A selective wait must preserve an unrelated publish until timeout.
    f.send(106, 0, 9, 0);
    wait(&f.room, |s| s.waiting && s.timer_deadline == Some(5));
    f.send(102, 2, 10, 44);
    wait(&f.room, |s| {
        s.waiting && s.scanned_prefix == 1 && s.timer_deadline == Some(5)
    });
    assert_eq!(f.collector.snapshot().progress, 8);
    f.client.advance_to(5).unwrap();
    f.expect(row(6, 2, 1, 2, 0, 1, 1, 9, 0));
    f.expect(row(7, 2, 1, 3, 0, 1, 1, 10, 244));
    f.check_transcript();
    assert_eq!(f.room.snapshot().timeouts, 1);
    f.send(103, 2, 11, 0);
    f.expect(row(10, 0, 1, 3, 1, 1, 1, 11, 0));
    f.check_transcript();
    wait(&f.room, |s| s.progress == 2 && s.waiting);
    f.send(103, 2, 12, 0);
    f.expect(row(3, 0, 1, 3, 1, 1, 2, 12, 0));
    f.check_transcript();
    f.send(103, 9, 13, 0);
    f.expect(row(5, 0, 1, 3, 1, 1, 2, 13, 0));
    f.check_transcript();
    f.send(105, 0, 14, 0);
    f.expect(row(1, 0, 1, 3, 1, 1, 2, 14, 0));
    f.check_transcript();
    f.send(107, 0, 15, 0);
    f.expect(row(9, 0, 1, 3, 1, 1, 2, 15, 0));
    let final_snapshot = wait(&f.collector, |s| s.progress == 15 && s.waiting);
    assert_eq!(final_snapshot.result, f.digest);
    f.observations.push([
        final_snapshot.progress,
        final_snapshot.result,
        final_snapshot.stage,
    ]);
    let expected = f.expected.clone();
    let checkpoints = f.observations.clone();
    let digest = final_snapshot.result;
    let records = final_snapshot.progress;
    f.finish(1);

    // Room death with a held timed request and nineteen queued requests.
    let mut forced = Fixture::new(allowance);
    forced.send(106, 0, 1, 0);
    wait(&forced.room, |s| s.waiting && s.timer_deadline == Some(5));
    for sequence in 2..21 {
        forced.send(102, 2, sequence, sequence);
    }
    wait(&forced.room, |s| s.waiting && s.scanned_prefix == 19);
    assert_eq!(forced.room.kill(), KillReceipt::Accepted);
    done(&forced.room, 2);
    // Only collector, independent computation and its observer may remain.
    let deadline = Instant::now() + Duration::from_secs(30);
    while forced.client.live_count() != 3 {
        assert!(Instant::now() < deadline, "correctness watchdog");
        std::thread::yield_now();
    }
    assert_eq!(forced.collector.snapshot().progress, 0);
    assert_eq!(forced.client.active_monitors(), 1);
    assert_eq!(forced.client.clock_snapshot().pending, 0);
    let cleanup = forced.room.snapshot();
    assert_eq!((cleanup.cleanup_messages, cleanup.cleanup_batches), (19, 3));
    forced.finish(2);

    // Host shutdown also settles a room held in receive plus queued requests.
    let mut closing = Fixture::new(allowance);
    closing.send(106, 0, 1, 0);
    wait(&closing.room, |s| s.waiting && s.timer_deadline == Some(5));
    for sequence in 2..21 {
        closing.send(102, 1, sequence, sequence);
    }
    wait(&closing.room, |s| s.waiting && s.scanned_prefix == 19);
    closing.runtime.shutdown();
    let closed_room = done(&closing.room, 2);
    let closed_collector = done(&closing.collector, 2);
    let closed_hog = done(&closing.hog, 2);
    let watcher = wait(&closing.watcher, |s| s.cleanup_event > 0);
    assert!([1, 2].contains(&watcher.terminal));
    if watcher.terminal == 1 {
        assert_eq!(watcher.result, 2);
    }
    for (snapshot, bytes) in [
        (&closed_room, 8),
        (&closed_collector, 0),
        (&closed_hog, 64),
        (&watcher, 0),
    ] {
        assert_eq!(
            (snapshot.drops, snapshot.live_bytes, snapshot.freed_bytes),
            (1, 0, bytes)
        );
    }
    assert_eq!(
        (
            closing.client.live_count(),
            closing.client.active_monitors(),
            closing.client.active_links()
        ),
        (0, 0, 0)
    );
    assert_eq!(closing.client.clock_snapshot().pending, 0);
    released(&closing.receipts, 88);
    println!("{{\"kind\":\"room_controls\",\"passed\":true,\"allowance\":{allowance},\"expected_transcript\":{expected:?},\"observed_records\":{records},\"observed_digest\":{digest},\"observed_checkpoints\":{checkpoints:?},\"two_session_membership\":true,\"idempotent_join_leave\":true,\"session_failure_isolation\":true,\"exact_incarnation_down\":true,\"peer_progress_with_hog\":true,\"selective_timeout_preserves_publish\":true,\"room_death_observed_by_sessions\":true,\"held_and_queued_ownership\":true,\"host_shutdown_settled\":true,\"speed_tests\":false}}");
}
