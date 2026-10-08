use super::*;
use bream_replacement_runtime::{ProcessId, RuntimeClient};

#[process_module]
mod target {
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
mod observer {
    fn run() {
        let reference = monitor(input_target());
        mark(10);
        let down = receive_down(reference);
        if down_ref(down) != reference || down_target(down) != input_target() {
            result(999);
            return;
        }
        result(down_reason(down));
    }
}
#[process_module]
mod self_monitor {
    fn run() {
        let first = monitor(self_id());
        let second = monitor(self_id());
        let copied = first;
        if first == second || first != copied {
            result(999);
            return;
        }
        match receive_down_for(first, 0) {
            DownResult::Down(unexpected) => {
                result(down_reason(unexpected));
                return;
            }
            DownResult::Timeout => {}
        }
        demonitor(first);
        demonitor_flush(second);
        result(1);
    }
}
#[process_module]
mod ordered_events {
    fn checked_reason(down: Down, reference: MonitorRef) -> u64 {
        if down_ref(down) != reference || down_target(down) != input_target() {
            return 999;
        }
        return down_reason(down);
    }
    fn run() {
        let first = monitor(input_target());
        let second = monitor(input_target());
        if first == second {
            result(999);
            return;
        }
        mark(20);
        let gate = receive(9);
        let mut head: u64 = 0;
        match receive_event() {
            Event::Message(message) => {
                if message_tag(message) != 7 {
                    result(999);
                    return;
                }
                let data = message_words(message);
                head = read(data, 0);
            }
            Event::Shared(unexpected_shared_1) => {
                result(999);
                return;
            }
            Event::Exit(unexpected_exit_1) => {
                result(999);
                return;
            }
            Event::Down(unexpected_head) => {
                result(999);
                return;
            }
        }
        // Selecting reference two skips reference one's earlier Down.
        let selected = receive_down(second);
        let reason_two = checked_reason(selected, second);
        let mut reason_one: u64 = 0;
        match receive_event() {
            Event::Shared(unexpected_shared_2) => {
                result(999);
                return;
            }
            Event::Exit(unexpected_exit_2) => {
                result(999);
                return;
            }
            Event::Down(first_down) => {
                reason_one = checked_reason(first_down, first);
            }
            Event::Message(unexpected_middle) => {
                result(999);
                return;
            }
        }
        match receive_event() {
            Event::Message(last_message) => {
                if message_tag(last_message) != 8 {
                    result(999);
                    return;
                }
                let last_data = message_words(last_message);
                result(head * 1000 + reason_two * 100 + reason_one * 10 + read(last_data, 0));
            }
            Event::Shared(unexpected_shared_3) => {
                result(999);
                return;
            }
            Event::Exit(unexpected_exit_3) => {
                result(999);
                return;
            }
            Event::Down(unexpected_tail) => {
                result(999);
            }
        }
    }
}
#[process_module]
mod cancellation {
    fn valid(down: Down, reference: MonitorRef) -> bool {
        return down_ref(down) == reference
            && down_target(down) == input_target()
            && down_reason(down) == 2;
    }
    fn run() {
        let removed = monitor(input_target());
        let retained = monitor(input_target());
        mark(30);
        let gate = receive(9);
        if input_argument() == 1 {
            demonitor_flush(removed);
        } else {
            demonitor(removed);
        }
        let mut observed: u64 = 0;
        match receive_down_for(removed, 0) {
            DownResult::Down(selected) => {
                if !valid(selected, removed) {
                    result(999);
                    return;
                }
                observed = down_reason(selected);
            }
            DownResult::Timeout => {}
        }
        let other = receive_down(retained);
        if !valid(other, retained) {
            result(999);
            return;
        }
        let first_words = message_words(receive_any());
        let last_words = message_words(receive_any());
        match receive_event_for(0) {
            EventResult::Event(extra) => {
                result(999);
            }
            EventResult::Timeout => {
                result(observed * 1000 + read(first_words, 0) * 100 + read(last_words, 0));
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
        match state.event {
            Event::Message(message) => {
                let data = message_words(message);
                result(read(data, 0) + len(state.input) + message_tag(gate));
            }
            Event::Shared(unexpected_shared_4) => {
                result(999);
                return;
            }
            Event::Exit(unexpected_exit_4) => {
                result(999);
                return;
            }
            Event::Down(unexpected) => {
                result(999);
            }
        }
    }
    fn run() {
        let state = State {
            event: receive_event(),
            input: owned_input(),
        };
        hold(state);
    }
}
#[process_module]
mod race_create {
    fn run() {
        mark(1);
        let gate = receive(9);
        let reference = monitor(input_target());
        let down = receive_down(reference);
        if down_ref(down) != reference || down_target(down) != input_target() {
            result(999);
            return;
        }
        result(down_reason(down));
    }
}
#[process_module]
mod race_cancel {
    fn run() {
        mark(1);
        let gate = receive(9);
        let mut count: u64 = 0;
        // The host releases this gate only after target cleanup and cancellation.
        // Zero-tick selection settles deterministically without a wall-clock oracle.
        for attempt in 0..2 {
            match receive_event_for(0) {
                EventResult::Event(event) => match event {
                    Event::Shared(unexpected_shared_5) => {
                        result(999);
                        return;
                    }
                    Event::Exit(unexpected_exit_5) => {
                        result(999);
                        return;
                    }
                    Event::Down(down) => {
                        if down_target(down) != input_target() || down_reason(down) != 2 {
                            result(999);
                            return;
                        }
                        count += 1;
                    }
                    Event::Message(unexpected) => {
                        result(999);
                        return;
                    }
                },
                EventResult::Timeout => {}
            }
        }
        result(count);
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
    // The observer registers while the target is live. The gate, not elapsed
    // time, determines whether normal completion, kill or failure happens next.
    for reason in [1, 2, 3] {
        let rt = Runtime::new(allowance);
        let client = rt.client();
        let watched = rt.spawn_input(target::program(), input(reason)).unwrap();
        gated(&watched, 1);
        let watcher = rt
            .spawn_to(observer::program(), input(0), watched.id())
            .unwrap();
        gated(&watcher, 10);
        assert_eq!(client.active_monitors(), 1);
        if reason == 2 {
            assert_eq!(watched.kill(), KillReceipt::Accepted);
        } else {
            signal(&client, watched.id(), 500);
        }
        done(&watched, reason as u8);
        assert_eq!(done(&watcher, 1).result, reason);
        assert_eq!(client.active_monitors(), 0);
        rt.shutdown();
    }

    // A retired identity remains absent after its slot is reused. A foreign
    // runtime's live identity is likewise absent in this local prototype.
    let rt = Runtime::with_capacity(allowance, 4);
    let client = rt.client();
    let retired = rt.spawn(peer::program(), vec![]).unwrap();
    done(&retired, 1);
    let dead = rt
        .spawn_to(observer::program(), input(0), retired.id())
        .unwrap();
    assert_eq!(done(&dead, 1).result, 4);
    let replacement = rt.spawn(target::program(), vec![]).unwrap();
    gated(&replacement, 1);
    assert_eq!(retired.id().slot(), replacement.id().slot());
    assert_ne!(retired.id(), replacement.id());
    let stale = rt
        .spawn_to(observer::program(), input(0), retired.id())
        .unwrap();
    assert_eq!(done(&stale, 1).result, 4);
    assert_eq!(replacement.snapshot().terminal, 0);
    let foreign_rt = Runtime::new(allowance);
    let foreign = foreign_rt.spawn(target::program(), vec![]).unwrap();
    gated(&foreign, 1);
    let absent = rt
        .spawn_to(observer::program(), input(0), foreign.id())
        .unwrap();
    assert_eq!(done(&absent, 1).result, 4);
    assert_eq!(foreign.snapshot().terminal, 0);
    let own = rt.spawn(self_monitor::program(), vec![]).unwrap();
    assert_eq!(done(&own, 1).result, 1);
    assert_eq!(client.active_monitors(), 0);
    foreign_rt.shutdown();
    done(&foreign, 2);
    rt.shutdown();
    done(&replacement, 2);

    // Both application messages and genuine Down notifications share arrival
    // order. Fixed-reference selection restores the skipped prefix afterward.
    let rt = Runtime::new(allowance);
    let client = rt.client();
    let watched = rt.spawn(target::program(), vec![]).unwrap();
    gated(&watched, 1);
    let watcher = rt
        .spawn_to(ordered_events::program(), input(0), watched.id())
        .unwrap();
    gated(&watcher, 20);
    assert_eq!(client.active_monitors(), 2);
    let (first, first_receipt) = packet(7, vec![10]);
    client.send(watcher.id(), first);
    assert_eq!(watched.kill(), KillReceipt::Accepted);
    done(&watched, 2);
    wait(&watcher, |s| s.waiting && s.queued == 3);
    let (last, last_receipt) = packet(8, vec![20]);
    client.send(watcher.id(), last);
    wait(&watcher, |s| s.waiting && s.queued == 4);
    signal(&client, watcher.id(), 9);
    assert_eq!(done(&watcher, 1).result, 10240);
    released(&[first_receipt, last_receipt], 8);
    assert_eq!(client.active_monitors(), 0);
    rt.shutdown();

    // A queued Down survives ordinary demonitor. Flush removes that reference
    // only, preserving another genuine Down and the application-message prefix.
    for flush in [false, true] {
        let rt = Runtime::new(allowance);
        let client = rt.client();
        let watched = rt.spawn(target::program(), vec![]).unwrap();
        gated(&watched, 1);
        let watcher = rt
            .spawn_to(
                cancellation::program(),
                input(u64::from(flush)),
                watched.id(),
            )
            .unwrap();
        gated(&watcher, 30);
        let (first, first_receipt) = packet(7, vec![10]);
        client.send(watcher.id(), first);
        assert_eq!(watched.kill(), KillReceipt::Accepted);
        done(&watched, 2);
        wait(&watcher, |s| s.waiting && s.queued == 3);
        assert_eq!(client.active_monitors(), 0);
        let (last, last_receipt) = packet(8, vec![20]);
        client.send(watcher.id(), last);
        wait(&watcher, |s| s.waiting && s.queued == 4);
        signal(&client, watcher.id(), 9);
        assert_eq!(done(&watcher, 1).result, if flush { 1020 } else { 3020 });
        released(&[first_receipt, last_receipt], 8);
        rt.shutdown();
    }

    // Observer death removes its outgoing relationships. Runtime shutdown owns
    // both sides even when their applications remain blocked indefinitely.
    for shutdown in [false, true] {
        let rt = Runtime::new(allowance);
        let client = rt.client();
        let watched = rt.spawn(target::program(), vec![]).unwrap();
        gated(&watched, 1);
        let watcher = rt
            .spawn_to(observer::program(), input(0), watched.id())
            .unwrap();
        gated(&watcher, 10);
        assert_eq!(client.active_monitors(), 1);
        if shutdown {
            rt.shutdown();
        } else {
            assert_eq!(watcher.kill(), KillReceipt::Accepted);
            done(&watcher, 2);
            assert_eq!(client.active_monitors(), 0);
            assert_eq!(watched.kill(), KillReceipt::Accepted);
            done(&watched, 2);
            rt.shutdown();
        }
        done(&watcher, 2);
        done(&watched, 2);
        assert_eq!(client.active_monitors(), 0);
    }

    // An owning Event may cross a generated helper's suspension. Both normal
    // resumption and kill release its message and the process-owned input once.
    for kill in [false, true] {
        let rt = Runtime::paused(allowance);
        let client = rt.client();
        let h = rt.spawn(held_event::program(), vec![5]).unwrap();
        let (message, receipt) = packet(7, vec![70]);
        client.send(h.id(), message);
        rt.start();
        gated(&h, 70);
        assert_eq!(receipt.drops(), 0);
        if kill {
            assert_eq!(h.kill(), KillReceipt::Accepted);
        } else {
            signal(&client, h.id(), 99);
        }
        let s = done(&h, if kill { 2 } else { 1 });
        if !kill {
            assert_eq!(s.result, 170);
        }
        assert_eq!((s.drops, s.live_bytes, s.freed_bytes), (1, 0, 8));
        released(&[receipt], 8);
        rt.shutdown();
    }

    let rt = Runtime::new(allowance);
    let client = rt.client();
    let mut creation_live = 0;
    let mut creation_absent = 0;
    for _ in 0..16 {
        let watched = rt.spawn(target::program(), vec![]).unwrap();
        gated(&watched, 1);
        let watcher = rt
            .spawn_to(race_create::program(), input(0), watched.id())
            .unwrap();
        gated(&watcher, 1);
        let barrier = Arc::new(Barrier::new(3));
        std::thread::scope(|scope| {
            let barrier = &barrier;
            let client = &client;
            let id = watcher.id();
            let target_handle = &watched;
            scope.spawn(move || {
                barrier.wait();
                signal(client, id, 9);
            });
            scope.spawn(move || {
                barrier.wait();
                assert_eq!(target_handle.kill(), KillReceipt::Accepted);
            });
            barrier.wait();
        });
        done(&watched, 2);
        match done(&watcher, 1).result {
            2 => creation_live += 1,
            4 => creation_absent += 1,
            other => panic!("creation/kill produced invalid reason or identity: {other}"),
        }
        assert_eq!(client.active_monitors(), 0);
    }
    let mut cancelled_before_conversion = 0;
    let mut converted_before_cancel = 0;
    for _ in 0..16 {
        let watched = rt.spawn(target::program(), vec![]).unwrap();
        gated(&watched, 1);
        let watcher = rt
            .spawn_to(race_cancel::program(), input(0), watched.id())
            .unwrap();
        gated(&watcher, 1);
        let reference = client.monitor(watcher.id(), watched.id()).unwrap();
        let barrier = Arc::new(Barrier::new(3));
        let origin_removed = std::thread::scope(|scope| {
            let barrier = &barrier;
            let client = &client;
            let id = watcher.id();
            let target_handle = &watched;
            let cancel = scope.spawn(move || {
                barrier.wait();
                client.demonitor_info(id, reference)
            });
            scope.spawn(move || {
                barrier.wait();
                assert_eq!(target_handle.kill(), KillReceipt::Accepted);
            });
            barrier.wait();
            cancel.join().unwrap()
        });
        done(&watched, 2);
        assert_eq!(client.active_monitors(), 0);
        let expected = if origin_removed { 0 } else { 1 };
        wait(&watcher, |s| s.waiting && s.queued == expected);
        signal(&client, watcher.id(), 9);
        assert_eq!(done(&watcher, 1).result, expected as u64);
        if origin_removed {
            cancelled_before_conversion += 1;
        } else {
            converted_before_cancel += 1;
        }
    }
    rt.shutdown();
    println!("{{\"kind\":\"monitor_controls\",\"passed\":true,\"allowance\":{allowance},\"normal_kill_failure_reasons\":true,\"absent_stale_foreign\":true,\"exact_generated_reference_and_target\":true,\"self_monitor_no_relationship\":true,\"repeated_fresh_references\":true,\"shared_event_order\":true,\"selective_reference\":true,\"plain_cancel_preserves_queued\":true,\"flush_preserves_other_events\":true,\"observer_death_cleanup\":true,\"shutdown_cleanup\":true,\"owning_event_helper_cleanup\":true,\"creation_kill_races\":16,\"creation_live\":{creation_live},\"creation_absent\":{creation_absent},\"cancel_kill_races\":16,\"cancelled_before_conversion\":{cancelled_before_conversion},\"converted_before_cancel\":{converted_before_cancel},\"speed_tests\":false}}");
}
