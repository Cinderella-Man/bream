use super::*;
use bream_spawn_runtime::{LinkError, ProcessId, RuntimeClient};

#[process_module]
mod victim {
    fn run() {
        mark(1);
        let gate = receive(500);
        if input_argument() == 3 {
            let empty = message_words(gate);
            result(read(empty, 0));
        } else {
            result(message_tag(gate));
        }
    }
}
#[process_module]
mod linked {
    fn run() {
        trap_exit(input_argument() == 1);
        link(input_target());
        link(input_target());
        mark(10);
        if input_argument() == 1 {
            let notification = receive_exit(input_target());
            if exit_from(notification) != input_target() {
                result(999);
                return;
            }
            result(exit_reason(notification));
        } else {
            let gate = receive(9);
            mark(11);
            result(message_tag(gate));
        }
    }
}
#[process_module]
mod own_link {
    fn run() {
        link(self_id());
        link(self_id());
        unlink(self_id());
        match try_link(self_id()) {
            LinkResult::Linked => {}
            LinkResult::NoProcess => {
                result(999);
                return;
            }
        }
        match receive_exit_for(self_id(), 0) {
            ExitResult::Exit(unexpected) => {
                result(999);
            }
            ExitResult::Timeout => {
                result(1);
            }
        }
    }
}
#[process_module]
mod forced {
    fn run() {
        trap_exit(true);
        mark(10);
        let gate = receive(9);
        mark(11);
        result(message_tag(gate));
    }
}
#[process_module]
mod absent {
    fn run() {
        mark(42);
        if input_argument() == 0 {
            match try_link(input_target()) {
                LinkResult::Linked => {
                    result(999);
                    return;
                }
                LinkResult::NoProcess => {}
            }
            match receive_exit_for(input_target(), 0) {
                ExitResult::Exit(unexpected) => {
                    result(999);
                }
                ExitResult::Timeout => {
                    result(44);
                }
            }
        } else {
            if input_argument() == 2 {
                trap_exit(true);
                link(input_target());
                let notification = receive_exit(input_target());
                if exit_from(notification) != input_target() {
                    result(999);
                    return;
                }
                result(exit_reason(notification));
            } else {
                link(input_target());
                mark(99);
                result(999);
            }
        }
    }
}
#[process_module]
mod cancelled {
    fn run() {
        trap_exit(true);
        link(input_target());
        mark(20);
        let gate = receive(9);
        unlink(input_target());
        let mut reason: u64 = 0;
        match receive_exit_for(input_target(), 0) {
            ExitResult::Exit(notification) => {
                if exit_from(notification) != input_target() {
                    result(999);
                    return;
                }
                reason = exit_reason(notification);
            }
            ExitResult::Timeout => {}
        }
        match receive_event_for(0) {
            EventResult::Event(unexpected) => {
                result(999);
            }
            EventResult::Timeout => {
                result(reason);
            }
        }
    }
}
#[process_module]
mod ordered_events {
    fn checked_exit(notification: Exit) -> u64 {
        if exit_from(notification) != input_target() {
            return 999;
        }
        return exit_reason(notification);
    }
    fn checked_down(notification: Down, reference: MonitorRef) -> u64 {
        if down_target(notification) != input_target() || down_ref(notification) != reference {
            return 999;
        }
        return down_reason(notification);
    }
    fn run() {
        trap_exit(true);
        link(input_target());
        let reference = monitor(input_target());
        mark(30);
        let gate = receive(9);
        let mut selected_reason: u64 = 0;
        if input_argument() == 1 {
            selected_reason = checked_exit(receive_exit(input_target()));
        }
        let mut head: u64 = 0;
        match receive_event() {
            Event::Message(message) => {
                if message_tag(message) != 7 {
                    result(999);
                    return;
                }
                let words = message_words(message);
                head = read(words, 0);
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
        if input_argument() == 0 {
            match receive_event() {
                Event::Exit(notification) => {
                    selected_reason = checked_exit(notification);
                }
                Event::Message(early_message) => {
                    result(999);
                    return;
                }
                Event::Down(misordered_down) => {
                    result(999);
                    return;
                }
            }
        }
        let mut down_reason_value: u64 = 0;
        match receive_event() {
            Event::Down(notification_down) => {
                down_reason_value = checked_down(notification_down, reference);
            }
            Event::Message(tail_message) => {
                result(999);
                return;
            }
            Event::Exit(tail_exit) => {
                result(999);
                return;
            }
        }
        match receive_event_for(0) {
            EventResult::Event(extra) => {
                result(999);
            }
            EventResult::Timeout => {
                result(head * 100 + selected_reason * 10 + down_reason_value);
            }
        }
    }
}
#[process_module]
mod held_event {
    struct State {
        event: Event,
        input: Words,
    }
    fn hold(state: State) {
        mark(70);
        let gate = receive(99);
        mark(71);
        match state.event {
            Event::Message(message) => {
                let words = message_words(message);
                result(read(words, 0) + len(state.input) + message_tag(gate));
            }
            Event::Exit(unexpected_exit) => {
                result(999);
            }
            Event::Down(unexpected_down) => {
                result(999);
            }
        }
    }
    fn run() {
        link(input_target());
        let state = State {
            event: receive_event(),
            input: owned_input(),
        };
        hold(state);
    }
}
#[process_module]
mod race_observer {
    fn run() {
        trap_exit(true);
        mark(40);
        let gate = receive(9);
        let mut count: u64 = 0;
        let mut reason: u64 = 0;
        for attempt in 0..2 {
            match receive_exit_for(input_target(), 0) {
                ExitResult::Exit(notification) => {
                    if exit_from(notification) != input_target() {
                        result(999);
                        return;
                    }
                    count += 1;
                    reason = exit_reason(notification);
                }
                ExitResult::Timeout => {}
            }
        }
        match receive_event_for(0) {
            EventResult::Event(extra) => {
                result(999);
            }
            EventResult::Timeout => {
                result(count * 10 + reason);
            }
        }
    }
}
fn input(argument: u64) -> Input {
    Input {
        words: vec![],
        tag: 0,
        argument,
    }
}
fn signal(client: &RuntimeClient, id: ProcessId, tag: u64) {
    let (message, _) = packet(tag, vec![]);
    client.send(id, message);
}
fn gated(handle: &ProcessHandle, stage: u64) -> Snapshot {
    wait(handle, |s| s.stage == stage && s.waiting)
}

pub(super) fn run(allowance: u32) {
    assert!([1, 4, 32].contains(&allowance));

    // Gates choose the semantic order; watchdog elapsed time is not evidence.
    for trapping in [false, true] {
        for reason in [1, 2, 3] {
            let rt = Runtime::new(allowance);
            let client = rt.client();
            let watched = rt.spawn_input(victim::program(), input(reason)).unwrap();
            gated(&watched, 1);
            let watcher = rt
                .spawn_to(linked::program(), input(u64::from(trapping)), watched.id())
                .unwrap();
            gated(&watcher, 10);
            assert_eq!(client.active_links(), 1, "repeat links must share one edge");
            if reason == 2 {
                assert_eq!(watched.kill(), KillReceipt::Accepted);
            } else {
                signal(&client, watched.id(), 500);
            }
            assert_eq!(done(&watched, reason as u8).exit_reason, reason);
            if trapping {
                let snapshot = done(&watcher, 1);
                assert_eq!((snapshot.result, snapshot.exit_reason), (reason, 1));
            } else if reason == 1 {
                let still_live = gated(&watcher, 10);
                assert_eq!(
                    (
                        still_live.terminal,
                        still_live.pending_exit,
                        still_live.queued
                    ),
                    (0, 0, 0)
                );
                signal(&client, watcher.id(), 9);
                assert_eq!(done(&watcher, 1).result, 9);
            } else {
                let snapshot = done(&watcher, 3);
                assert_eq!(
                    (snapshot.exit_reason, snapshot.stage, snapshot.result),
                    (reason, 10, 0)
                );
            }
            assert_eq!(client.active_links(), 0);
            rt.shutdown();
        }
    }

    let rt = Runtime::new(allowance);
    let client = rt.client();
    let own = rt.spawn(own_link::program(), vec![]).unwrap();
    assert_eq!(done(&own, 1).result, 1);
    assert_eq!(client.active_links(), 0);
    let forced_handle = rt.spawn(forced::program(), vec![]).unwrap();
    assert!(gated(&forced_handle, 10).trap_exit);
    assert_eq!(client.link(forced_handle.id(), forced_handle.id()), Ok(()));
    assert_eq!(client.active_links(), 0);
    assert_eq!(forced_handle.kill(), KillReceipt::Accepted);
    let snapshot = done(&forced_handle, 2);
    assert_eq!(
        (snapshot.exit_reason, snapshot.stage, snapshot.result),
        (2, 10, 0)
    );
    rt.shutdown();

    // An exact retired identity stays absent after reuse; a foreign identity is
    // absent locally as well. Recovery leaves no asynchronous Exit behind.
    let rt = Runtime::with_capacity(allowance, 4);
    let client = rt.client();
    let retired = rt.spawn(peer::program(), vec![]).unwrap();
    done(&retired, 1);
    let replacement = rt.spawn(victim::program(), vec![]).unwrap();
    gated(&replacement, 1);
    assert_eq!(retired.id().slot(), replacement.id().slot());
    assert_ne!(retired.id(), replacement.id());
    let foreign_rt = Runtime::new(allowance);
    let foreign = foreign_rt.spawn(victim::program(), vec![]).unwrap();
    gated(&foreign, 1);
    assert_eq!(
        client.link(retired.id(), replacement.id()),
        Err(LinkError::OriginNotLive)
    );
    for missing in [retired.id(), foreign.id()] {
        for mode in [0, 1, 2] {
            let watcher = rt
                .spawn_to(absent::program(), input(mode), missing)
                .unwrap();
            if mode == 1 {
                let snapshot = done(&watcher, 3);
                assert_eq!(
                    (snapshot.exit_reason, snapshot.stage, snapshot.result),
                    (4, 42, 0)
                );
            } else {
                let snapshot = done(&watcher, 1);
                assert_eq!(snapshot.result, if mode == 0 { 44 } else { 4 });
                assert_eq!(snapshot.queued, 0);
            }
            assert_eq!(client.active_links(), 0);
        }
    }
    assert_eq!(replacement.snapshot().terminal, 0);
    assert_eq!(foreign.snapshot().terminal, 0);
    foreign_rt.shutdown();
    done(&foreign, 2);
    rt.shutdown();
    done(&replacement, 2);

    // Removing the relationship before death suppresses conversion; removing
    // it after conversion leaves the already queued Exit intact.
    for before_death in [false, true] {
        let rt = Runtime::new(allowance);
        let client = rt.client();
        let watched = rt.spawn(victim::program(), vec![]).unwrap();
        gated(&watched, 1);
        let watcher = rt
            .spawn_to(cancelled::program(), input(0), watched.id())
            .unwrap();
        gated(&watcher, 20);
        if before_death {
            client.unlink(watcher.id(), watched.id());
            assert_eq!(client.active_links(), 0);
        }
        assert_eq!(watched.kill(), KillReceipt::Accepted);
        done(&watched, 2);
        let expected = if before_death { 0 } else { 1 };
        wait(&watcher, |s| s.waiting && s.queued == expected);
        signal(&client, watcher.id(), 9);
        assert_eq!(done(&watcher, 1).result, if before_death { 0 } else { 2 });
        assert_eq!(client.active_links(), 0);
        rt.shutdown();
    }

    for selective in [false, true] {
        let rt = Runtime::new(allowance);
        let client = rt.client();
        let watched = rt.spawn(victim::program(), vec![]).unwrap();
        gated(&watched, 1);
        let watcher = rt
            .spawn_to(
                ordered_events::program(),
                input(u64::from(selective)),
                watched.id(),
            )
            .unwrap();
        gated(&watcher, 30);
        assert_eq!((client.active_links(), client.active_monitors()), (1, 1));
        let (message, receipt) = packet(7, vec![10]);
        client.send(watcher.id(), message);
        assert_eq!(watched.kill(), KillReceipt::Accepted);
        done(&watched, 2);
        wait(&watcher, |s| s.waiting && s.queued == 3);
        signal(&client, watcher.id(), 9);
        assert_eq!(done(&watcher, 1).result, 1022);
        released(&[receipt], 8);
        assert_eq!((client.active_links(), client.active_monitors()), (0, 0));
        rt.shutdown();
    }

    for linked_failure in [false, true] {
        let rt = Runtime::paused(allowance);
        let client = rt.client();
        let watched = rt.spawn(victim::program(), vec![]).unwrap();
        let watcher = rt
            .spawn_to(
                held_event::program(),
                Input {
                    words: vec![5],
                    tag: 0,
                    argument: 0,
                },
                watched.id(),
            )
            .unwrap();
        let (message, receipt) = packet(7, vec![70]);
        client.send(watcher.id(), message);
        rt.start();
        gated(&watched, 1);
        gated(&watcher, 70);
        assert_eq!(receipt.drops(), 0);
        if linked_failure {
            assert_eq!(watched.kill(), KillReceipt::Accepted);
            done(&watched, 2);
        } else {
            signal(&client, watcher.id(), 99);
        }
        let snapshot = done(&watcher, if linked_failure { 3 } else { 1 });
        assert_eq!(
            (snapshot.drops, snapshot.live_bytes, snapshot.freed_bytes),
            (1, 0, 8)
        );
        if linked_failure {
            assert_eq!(
                (snapshot.exit_reason, snapshot.stage, snapshot.result),
                (2, 70, 0)
            );
        } else {
            assert_eq!((snapshot.result, snapshot.stage), (170, 71));
            assert_eq!(watched.kill(), KillReceipt::Accepted);
            done(&watched, 2);
        }
        released(&[receipt], 8);
        assert_eq!(client.active_links(), 0);
        rt.shutdown();
    }

    let rt = Runtime::new(allowance);
    let client = rt.client();
    let mut creation_live = 0;
    let mut creation_absent = 0;
    for _ in 0..16 {
        let watched = rt.spawn(victim::program(), vec![]).unwrap();
        gated(&watched, 1);
        let watcher = rt
            .spawn_to(race_observer::program(), input(0), watched.id())
            .unwrap();
        gated(&watcher, 40);
        assert_eq!(client.set_trap_exit(watcher.id(), true), Ok(true));
        let barrier = Arc::new(Barrier::new(3));
        std::thread::scope(|scope| {
            let barrier = &barrier;
            let client = &client;
            let origin = watcher.id();
            let target_id = watched.id();
            let target_handle = &watched;
            scope.spawn(move || {
                barrier.wait();
                assert_eq!(client.link(origin, target_id), Ok(()));
            });
            scope.spawn(move || {
                barrier.wait();
                assert_eq!(target_handle.kill(), KillReceipt::Accepted);
            });
            barrier.wait();
        });
        done(&watched, 2);
        wait(&watcher, |s| s.waiting && s.queued == 1);
        signal(&client, watcher.id(), 9);
        match done(&watcher, 1).result {
            12 => creation_live += 1,
            14 => creation_absent += 1,
            other => panic!("link/kill produced an invalid notification count or reason: {other}"),
        }
        assert_eq!(client.active_links(), 0);
    }
    let mut cancelled_before_conversion = 0;
    let mut converted_before_cancel = 0;
    for _ in 0..16 {
        let watched = rt.spawn(victim::program(), vec![]).unwrap();
        gated(&watched, 1);
        let watcher = rt
            .spawn_to(race_observer::program(), input(0), watched.id())
            .unwrap();
        gated(&watcher, 40);
        assert_eq!(client.link(watcher.id(), watched.id()), Ok(()));
        let barrier = Arc::new(Barrier::new(3));
        std::thread::scope(|scope| {
            let barrier = &barrier;
            let client = &client;
            let origin = watcher.id();
            let target_id = watched.id();
            let target_handle = &watched;
            scope.spawn(move || {
                barrier.wait();
                client.unlink(origin, target_id);
            });
            scope.spawn(move || {
                barrier.wait();
                assert_eq!(target_handle.kill(), KillReceipt::Accepted);
            });
            barrier.wait();
        });
        done(&watched, 2);
        assert_eq!(client.active_links(), 0);
        signal(&client, watcher.id(), 9);
        match done(&watcher, 1).result {
            0 => cancelled_before_conversion += 1,
            12 => converted_before_cancel += 1,
            other => {
                panic!("unlink/kill produced an invalid notification count or reason: {other}")
            }
        }
    }
    rt.shutdown();
    println!("{{\"kind\":\"link_controls\",\"passed\":true,\"allowance\":{allowance},\"self_and_repeated_links\":true,\"normal_untrapped_ignored\":true,\"normal_kill_failure_trapping\":true,\"fatal_link_wakes_parked\":true,\"host_kill_untrappable\":true,\"absent_try_link_recovery\":true,\"uncaught_noproc_reason\":true,\"trapped_noproc_notification\":true,\"stale_foreign_isolation\":true,\"unlink_before_death\":true,\"unlink_preserves_converted_exit\":true,\"application_exit_down_order\":true,\"selective_exit_preserves_prefix\":true,\"owning_event_helper_cleanup\":true,\"creation_kill_races\":16,\"creation_live\":{creation_live},\"creation_absent\":{creation_absent},\"unlink_kill_races\":16,\"cancelled_before_conversion\":{cancelled_before_conversion},\"converted_before_cancel\":{converted_before_cancel},\"speed_tests\":false}}");
}
