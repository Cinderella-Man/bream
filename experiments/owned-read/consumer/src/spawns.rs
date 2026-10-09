use super::*;
use bream_read_runtime::{SpawnError, SpawnErrorKind};

#[process_module]
mod monitored_parent {
    struct Waiting {
        child: Spawned,
        words: Words,
    }
    fn child() {
        let inherited = trap_exit(false);
        if inherited {
            send(input_target(), 999, owned_input());
            return;
        }
        if input_argument() == 3 {
            let words = owned_input();
            result(read(words, 99));
        } else {
            send(input_target(), input_tag(), owned_input());
        }
    }
    fn await_reason(state: Waiting) {
        let notification = receive_down(spawned_ref(state.child));
        if down_ref(notification) != spawned_ref(state.child)
            || down_target(notification) != spawned_pid(state.child)
        {
            result(999);
            return;
        }
        result(read(state.words, 0) * 10 + down_reason(notification));
    }
    fn run() {
        trap_exit(true);
        let child_result: Spawned = spawn_monitor(child, owned_input(), 71, input_argument());
        if input_argument() == 3 {
            let notification = receive_down(spawned_ref(child_result));
            if down_target(notification) != spawned_pid(child_result) {
                result(999);
                return;
            }
            result(down_reason(notification));
        } else {
            let message = receive_any();
            if message_tag(message) != 71 {
                result(999);
                return;
            }
            let state = Waiting {
                child: child_result,
                words: message_words(message),
            };
            await_reason(state);
        }
    }
}
#[process_module]
mod linked_parent {
    fn child() {
        if input_argument() == 3 {
            let words = owned_input();
            result(read(words, 99));
        } else {
            send(input_target(), 71, owned_input());
        }
    }
    fn run() {
        trap_exit(input_tag() == 1);
        let child_pid = spawn_link(child, owned_input(), 0, input_argument());
        if input_tag() == 1 {
            let notification = receive_exit(child_pid);
            if exit_from(notification) != child_pid {
                result(999);
                return;
            }
            result(exit_reason(notification));
        } else {
            mark(50);
            let message = receive_any();
            let words = message_words(message);
            result(read(words, 0));
        }
    }
}
#[process_module]
mod combined_parent {
    fn child() {
        send(input_target(), 71, owned_input());
        if input_argument() == 3 {
            let empty = owned_input();
            result(read(empty, 0));
        }
    }
    fn run() {
        trap_exit(true);
        let child_result = spawn_link_monitor(child, owned_input(), 0, input_argument());
        let mut value: u64 = 0;
        match receive_event() {
            Event::Message(message) => {
                if message_tag(message) != 71 {
                    result(999);
                    return;
                }
                let words = message_words(message);
                value = read(words, 0);
            }
            Event::Shared(unexpected_shared_1) => {
                result(999);
                return;
            }
            Event::Exit(early_exit) => {
                result(999);
                return;
            }
            Event::Down(early_down) => {
                result(999);
                return;
            }
        }
        let mut reason: u64 = 0;
        match receive_event() {
            Event::Shared(unexpected_shared_2) => {
                result(999);
                return;
            }
            Event::Exit(notification) => {
                if exit_from(notification) != spawned_pid(child_result) {
                    result(999);
                    return;
                }
                reason = exit_reason(notification);
            }
            Event::Message(extra_message) => {
                result(999);
                return;
            }
            Event::Down(misordered_down) => {
                result(999);
                return;
            }
        }
        match receive_event() {
            Event::Down(notification_down) => {
                if down_target(notification_down) != spawned_pid(child_result)
                    || down_ref(notification_down) != spawned_ref(child_result)
                    || down_reason(notification_down) != reason
                {
                    result(999);
                    return;
                }
            }
            Event::Message(tail_message) => {
                result(999);
                return;
            }
            Event::Shared(unexpected_shared_3) => {
                result(999);
                return;
            }
            Event::Exit(extra_exit) => {
                result(999);
                return;
            }
        }
        match receive_event_for(0) {
            EventResult::Event(extra) => {
                result(999);
            }
            EventResult::Timeout => {
                result(value * 10 + reason);
            }
        }
    }
}
#[process_module]
mod no_capacity {
    fn child() {
        result(999);
    }
    fn run() {
        mark(60);
        let child_result = spawn_link_monitor(child, owned_input(), 0, 0);
        mark(61);
        result(999);
    }
}
#[process_module]
mod host_parent {
    fn run() {
        trap_exit(true);
        mark(1);
        let gate = receive(9);
        result(message_tag(gate));
    }
}
#[process_module]
mod host_child {
    fn run() {
        mark(2);
        let gate = receive(99);
        result(message_tag(gate));
    }
}
fn input(tag: u64, argument: u64) -> Input {
    Input {
        words: vec![42],
        tag,
        argument,
    }
}
fn rejected(error: SpawnError, expected: SpawnErrorKind) {
    assert_eq!(error.kind, expected);
    let snapshot = error
        .cleanup
        .expect("prepared input must be destroyed before rejection returns");
    assert_eq!(
        (
            snapshot.terminal,
            snapshot.drops,
            snapshot.live_bytes,
            snapshot.freed_bytes
        ),
        (4, 1, 0, 8)
    );
    assert!(snapshot.stop_event > 0 && snapshot.cleanup_event > snapshot.stop_event);
}
fn all_released(handle: &ProcessHandle) {
    let snapshot = wait(handle, |s| s.drops == 1);
    assert_eq!((snapshot.live_bytes, snapshot.freed_bytes), (0, 8));
}

pub(super) fn run(allowance: u32) {
    assert!([1, 4, 32].contains(&allowance));
    for reason in [1, 3] {
        let rt = Runtime::new(allowance);
        let client = rt.client();
        let parent = rt
            .spawn_input(monitored_parent::program(), input(0, reason))
            .unwrap();
        let snapshot = done(&parent, 1);
        assert_eq!(snapshot.result, if reason == 1 { 421 } else { 3 });
        all_released(&parent);
        assert_eq!((client.active_links(), client.active_monitors()), (0, 0));
        rt.shutdown();

        for trapping in [false, true] {
            let rt = Runtime::new(allowance);
            let client = rt.client();
            let parent = rt
                .spawn_input(linked_parent::program(), input(u64::from(trapping), reason))
                .unwrap();
            let snapshot = done(&parent, if !trapping && reason == 3 { 3 } else { 1 });
            if trapping {
                assert_eq!(snapshot.result, reason);
            } else if reason == 1 {
                assert_eq!(snapshot.result, 42);
            } else {
                assert_eq!((snapshot.exit_reason, snapshot.result), (3, 0));
            }
            all_released(&parent);
            assert_eq!((client.active_links(), client.active_monitors()), (0, 0));
            rt.shutdown();
        }
        let rt = Runtime::new(allowance);
        let client = rt.client();
        let parent = rt
            .spawn_input(combined_parent::program(), input(0, reason))
            .unwrap();
        assert_eq!(done(&parent, 1).result, 420 + reason);
        all_released(&parent);
        assert_eq!((client.active_links(), client.active_monitors()), (0, 0));
        rt.shutdown();
    }

    // A failed authored publication destroys the moved input once and never
    // reaches the next checked statement. The caller's allocation receipt stays
    // attached to the transferred buffer, including when the child rejects it.
    let rt = Runtime::with_capacity(allowance, 1);
    let client = rt.client();
    let parent = rt.spawn_input(no_capacity::program(), input(0, 0)).unwrap();
    let snapshot = done(&parent, 3);
    assert_eq!(
        (snapshot.stage, snapshot.result, snapshot.exit_reason),
        (60, 0, 3)
    );
    all_released(&parent);
    assert_eq!((client.active_links(), client.active_monitors()), (0, 0));
    rt.shutdown();

    // Full identity and publication failure: there is no relationship left
    // behind for a foreign, retired, capacity-rejected or closed origin.
    let rt = Runtime::with_capacity(allowance, 1);
    let client = rt.client();
    let parent = rt.spawn(host_parent::program(), vec![]).unwrap();
    wait(&parent, |s| s.stage == 1 && s.waiting);
    rejected(
        client
            .spawn_link_monitor(parent.id(), host_child::program(), input(0, 0))
            .err()
            .unwrap(),
        SpawnErrorKind::Capacity,
    );
    assert_eq!((client.active_links(), client.active_monitors()), (0, 0));
    assert_eq!(parent.kill(), KillReceipt::Accepted);
    done(&parent, 2);
    rejected(
        client
            .spawn_monitor(parent.id(), host_child::program(), input(0, 0))
            .err()
            .unwrap(),
        SpawnErrorKind::OriginNotLive,
    );
    let foreign_rt = Runtime::new(allowance);
    let foreign = foreign_rt.spawn(host_parent::program(), vec![]).unwrap();
    wait(&foreign, |s| s.stage == 1 && s.waiting);
    rejected(
        client
            .spawn_link(foreign.id(), host_child::program(), input(0, 0))
            .err()
            .unwrap(),
        SpawnErrorKind::OriginNotLive,
    );
    foreign_rt.shutdown();
    done(&foreign, 2);
    rt.shutdown();
    rejected(
        client
            .spawn_monitor(parent.id(), host_child::program(), input(0, 0))
            .err()
            .unwrap(),
        SpawnErrorKind::Closed,
    );
    assert_eq!((client.active_links(), client.active_monitors()), (0, 0));

    let mut parent_race_published = 0;
    let mut parent_race_rejected = 0;
    for _ in 0..16 {
        let rt = Runtime::new(allowance);
        let client = rt.client();
        let parent = rt.spawn(host_parent::program(), vec![]).unwrap();
        wait(&parent, |s| s.stage == 1 && s.waiting);
        let barrier = Arc::new(Barrier::new(3));
        let publication = std::thread::scope(|scope| {
            let barrier = &barrier;
            let client = &client;
            let parent_id = parent.id();
            let parent_handle = &parent;
            let publish = scope.spawn(move || {
                barrier.wait();
                client.spawn_link_monitor(parent_id, host_child::program(), input(0, 0))
            });
            scope.spawn(move || {
                barrier.wait();
                assert_eq!(parent_handle.kill(), KillReceipt::Accepted);
            });
            barrier.wait();
            publish.join().unwrap()
        });
        done(&parent, 2);
        match publication {
            Ok((child, _reference)) => {
                parent_race_published += 1;
                let snapshot = done(&child, 3);
                assert_eq!(snapshot.exit_reason, 2);
                assert!(!snapshot.trap_exit);
                all_released(&child);
            }
            Err(error) => {
                parent_race_rejected += 1;
                rejected(error, SpawnErrorKind::OriginNotLive);
            }
        }
        assert_eq!((client.active_links(), client.active_monitors()), (0, 0));
        rt.shutdown();
    }
    let mut shutdown_race_published = 0;
    let mut shutdown_race_rejected = 0;
    for _ in 0..16 {
        let rt = Runtime::new(allowance);
        let client = rt.client();
        let parent = rt.spawn(host_parent::program(), vec![]).unwrap();
        wait(&parent, |s| s.stage == 1 && s.waiting);
        let barrier = Arc::new(Barrier::new(3));
        let publication = std::thread::scope(|scope| {
            let barrier = &barrier;
            let client = &client;
            let parent_id = parent.id();
            let publish = scope.spawn(move || {
                barrier.wait();
                client.spawn_link_monitor(parent_id, host_child::program(), input(0, 0))
            });
            scope.spawn(move || {
                barrier.wait();
                rt.shutdown();
            });
            barrier.wait();
            publish.join().unwrap()
        });
        done(&parent, 2);
        match publication {
            Ok((child, _reference)) => {
                shutdown_race_published += 1;
                let snapshot = wait(&child, |s| s.cleanup_event > 0);
                assert!([2, 3].contains(&snapshot.terminal));
                assert_eq!(snapshot.exit_reason, 2);
                all_released(&child);
            }
            Err(error) => {
                shutdown_race_rejected += 1;
                rejected(error, SpawnErrorKind::Closed);
            }
        }
        assert_eq!((client.active_links(), client.active_monitors()), (0, 0));
    }
    println!("{{\"kind\":\"spawn_controls\",\"passed\":true,\"allowance\":{allowance},\"immediate_monitor_reasons\":true,\"linked_trap_and_fatal\":true,\"combined_application_exit_down_order\":true,\"child_trap_defaults_false\":true,\"owning_spawned_helper\":true,\"moved_input_released_once\":true,\"checked_failure_stops_caller\":true,\"publication_failure_cleanup\":true,\"stale_foreign_origin_rejection\":true,\"spawn_parent_kill_races\":16,\"parent_race_published\":{parent_race_published},\"parent_race_rejected\":{parent_race_rejected},\"spawn_shutdown_races\":16,\"shutdown_race_published\":{shutdown_race_published},\"shutdown_race_rejected\":{shutdown_race_rejected},\"speed_tests\":false}}");
}
