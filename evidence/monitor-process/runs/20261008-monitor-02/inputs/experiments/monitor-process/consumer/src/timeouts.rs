use super::*;
use bream_monitor_runtime::ClockError;

#[process_module]
mod timed {
    fn run() {
        mark(1);
        match receive_for(input_tag(), input_argument()) {
            ReceiveResult::Message(message) => {
                let data = message_words(message);
                result(read(data, 0));
            }
            ReceiveResult::Timeout => {
                result(999);
            }
        }
        mark(2);
    }
}
#[process_module]
mod restore {
    fn run() {
        match receive_for(99, 0) {
            ReceiveResult::Message(message) => {
                result(message_tag(message));
                return;
            }
            ReceiveResult::Timeout => {}
        }
        let first = message_words(receive_any());
        let second = message_words(receive_any());
        result(read(first, 0) * 100 + read(second, 0));
    }
}
#[process_module]
mod two_waits {
    fn run() {
        let mut sum: u64 = 0;
        match receive_for(7, 5) {
            ReceiveResult::Message(message) => {
                let data = message_words(message);
                sum += read(data, 0);
            }
            ReceiveResult::Timeout => {
                sum += 100;
            }
        }
        mark(2);
        match receive_for(8, 10) {
            ReceiveResult::Message(second_message) => {
                sum += message_tag(second_message);
            }
            ReceiveResult::Timeout => {
                sum += 1000;
            }
        }
        result(sum);
    }
}
#[process_module]
mod evaluated {
    fn tag() -> u64 {
        mark(11);
        send(self_id(), 700, owned_input());
        return 7;
    }
    fn delay() -> u64 {
        let message = receive(700);
        let data = message_words(message);
        mark(22);
        return read(data, 0);
    }
    fn wait_once() -> ReceiveResult {
        return receive_for(tag(), delay());
    }
    fn run() {
        match wait_once() {
            ReceiveResult::Message(message) => {
                result(message_tag(message));
            }
            ReceiveResult::Timeout => {
                result(700);
            }
        }
    }
}
#[process_module]
mod held_result {
    struct State {
        selection: ReceiveResult,
        input: Words,
    }
    fn hold(state: State) {
        mark(70);
        match receive_any_for(5) {
            ReceiveResult::Message(message) => {
                result(message_tag(message));
            }
            ReceiveResult::Timeout => match state.selection {
                ReceiveResult::Message(selected) => {
                    let data = message_words(selected);
                    result(read(data, 0) + len(state.input));
                }
                ReceiveResult::Timeout => {
                    result(999);
                }
            },
        }
    }
    fn run() {
        let state = State {
            selection: receive_for(7, 0),
            input: owned_input(),
        };
        hold(state);
    }
}
#[process_module]
mod nested_braced {
    fn run() {
        match receive_for(7, 0) {
            ReceiveResult::Message(first) => {
                result(message_tag(first));
            }
            ReceiveResult::Timeout => {
                match receive_for(8, 0) {
                    ReceiveResult::Message(second) => {
                        result(message_tag(second));
                    }
                    ReceiveResult::Timeout => {
                        result(999);
                    }
                };
            }
        }
    }
}
#[process_module]
mod nested_formatted {
    fn run() {
        match receive_for(7, 0) {
            ReceiveResult::Message(first) => {
                result(message_tag(first));
            }
            ReceiveResult::Timeout => match receive_for(8, 0) {
                ReceiveResult::Message(second) => {
                    result(message_tag(second));
                }
                ReceiveResult::Timeout => {
                    result(999);
                }
            },
        }
    }
}
fn input(tag: u64, argument: u64) -> Input {
    Input {
        words: vec![5],
        tag,
        argument,
    }
}
fn armed(handle: &ProcessHandle, generation: u64) -> Snapshot {
    wait(handle, |s| {
        s.waiting && s.timer_generation == Some(generation)
    })
}

pub(super) fn run(allowance: u32) {
    assert!([1, 4, 32].contains(&allowance));
    // Formatting must preserve the nested match's automatic scheduling charge.
    let rt = Runtime::new(allowance);
    let braced = done(&rt.spawn(nested_braced::program(), vec![]).unwrap(), 1);
    let formatted = done(&rt.spawn(nested_formatted::program(), vec![]).unwrap(), 1);
    assert_eq!(
        (braced.result, braced.steps, braced.polls, braced.timeouts),
        (
            formatted.result,
            formatted.steps,
            formatted.polls,
            formatted.timeouts
        ),
        "nested match formatting changed the checkpoint budget"
    );
    rt.shutdown();
    // Zero must finish the initial scan across generated turns before timing out.
    let rt = Runtime::paused(allowance);
    let client = rt.client();
    let h = rt.spawn_input(timed::program(), input(99, 0)).unwrap();
    let mut receipts = Vec::new();
    for _ in 0..128 {
        let (m, r) = packet(1, vec![1]);
        client.send(h.id(), m);
        receipts.push(r);
    }
    let (m, r) = packet(99, vec![123]);
    client.send(h.id(), m);
    receipts.push(r);
    let peer = rt.spawn(peer::program(), vec![]).unwrap();
    rt.start();
    let complete = done(&h, 1);
    let peer_complete = done(&peer, 1);
    assert_eq!(
        (complete.result, complete.scans, complete.timeouts),
        (123, 129, 0)
    );
    assert!(peer_complete.stop_event < complete.stop_event);
    assert_eq!(client.clock_snapshot().armed, 0);
    released(&receipts, 8);
    let empty = rt.spawn_input(timed::program(), input(99, 0)).unwrap();
    assert_eq!(
        (done(&empty, 1).result, empty.snapshot().timeouts),
        (999, 1)
    );
    rt.shutdown();

    let rt = Runtime::paused(allowance);
    let h = rt.spawn(restore::program(), vec![]).unwrap();
    let mut receipts = Vec::new();
    for (tag, value) in [(1, 10), (2, 20)] {
        let (m, r) = packet(tag, vec![value]);
        rt.client().send(h.id(), m);
        receipts.push(r);
    }
    rt.start();
    assert_eq!(done(&h, 1).result, 1020);
    released(&receipts, 8);
    rt.shutdown();

    // Unmatched arrivals preserve both the deadline and scan position.
    let rt = Runtime::new(allowance);
    let client = rt.client();
    let h = rt.spawn_input(timed::program(), input(7, 5)).unwrap();
    assert_eq!(armed(&h, 1).timer_deadline, Some(5));
    client.advance_to(4).unwrap();
    let mut receipts = Vec::new();
    for i in 1..=3 {
        let (m, r) = packet(1, vec![i]);
        client.send(h.id(), m);
        receipts.push(r);
        let s = wait(&h, |s| s.waiting && s.scans == i);
        assert_eq!(
            (s.timer_generation, s.timer_deadline, s.timeouts),
            (Some(1), Some(5), 0)
        );
    }
    client.advance_to(5).unwrap();
    assert_eq!((done(&h, 1).result, h.snapshot().timeouts), (999, 1));
    assert_eq!(
        (
            client.clock_snapshot().armed,
            client.clock_snapshot().expired,
            client.clock_snapshot().pending
        ),
        (1, 1, 0)
    );
    released(&receipts, 8);
    rt.shutdown();

    // Timer A is removed before timer B is installed, with a distinct generation.
    let rt = Runtime::new(allowance);
    let client = rt.client();
    let h = rt.spawn(two_waits::program(), vec![]).unwrap();
    armed(&h, 1);
    let (m, r) = packet(7, vec![10]);
    client.send(h.id(), m);
    assert_eq!(armed(&h, 2).timer_deadline, Some(10));
    client.advance_to(5).unwrap();
    done(&rt.spawn(peer::program(), vec![]).unwrap(), 1);
    assert_eq!(
        (h.snapshot().timer_generation, h.snapshot().timeouts),
        (Some(2), 0)
    );
    client.advance_to(10).unwrap();
    assert_eq!(done(&h, 1).result, 1010);
    assert_eq!(
        (
            client.clock_snapshot().armed,
            client.clock_snapshot().cancelled
        ),
        (2, 1)
    );
    released(&[r], 8);
    rt.shutdown();

    // Generated helper operands are evaluated once, in source order.
    let rt = Runtime::new(allowance);
    let client = rt.client();
    let h = rt.spawn(evaluated::program(), vec![5]).unwrap();
    assert_eq!((armed(&h, 1).stage, h.snapshot().drops), (22, 1));
    let (m, r) = packet(1, vec![1]);
    client.send(h.id(), m);
    assert_eq!(wait(&h, |s| s.waiting && s.scans == 2).stage, 22);
    client.advance_to(5).unwrap();
    assert_eq!(
        (done(&h, 1).result, h.snapshot().drops, h.snapshot().stage),
        (700, 1, 22)
    );
    released(&[r], 8);
    rt.shutdown();

    // An owning result held in a record/helper is released on timeout or kill.
    for kill in [false, true] {
        let rt = Runtime::paused(allowance);
        let client = rt.client();
        let h = rt.spawn(held_result::program(), vec![5]).unwrap();
        let (m, r) = packet(7, vec![70]);
        client.send(h.id(), m);
        rt.start();
        assert_eq!(armed(&h, 1).stage, 70);
        assert_eq!(r.drops(), 0);
        if kill {
            assert_eq!(h.kill(), KillReceipt::Accepted);
        } else {
            client.advance_to(5).unwrap();
        }
        let s = done(&h, if kill { 2 } else { 1 });
        assert_eq!(s.drops, 1);
        if !kill {
            assert_eq!(s.result, 71);
        }
        assert_eq!(client.clock_snapshot().pending, 0);
        released(&[r], 8);
        rt.shutdown();
    }

    // Waiting kill cancels the timer, frees queued data and isolates a reused slot.
    let rt = Runtime::with_capacity(allowance, 1);
    let client = rt.client();
    let old = rt.spawn_input(timed::program(), input(7, 50)).unwrap();
    armed(&old, 1);
    let mut receipts = Vec::new();
    for i in 0..33 {
        let (m, r) = packet(1, vec![i]);
        client.send(old.id(), m);
        receipts.push(r);
    }
    wait(&old, |s| s.waiting && s.scans == 33);
    assert_eq!(old.kill(), KillReceipt::Accepted);
    let stopped = done(&old, 2);
    assert_eq!(
        (
            stopped.cleanup_messages,
            stopped.timeouts,
            client.clock_snapshot().pending
        ),
        (33, 0, 0)
    );
    released(&receipts, 8);
    let next = rt.spawn_input(timed::program(), input(7, 100)).unwrap();
    assert_eq!(old.id().slot(), next.id().slot());
    assert_ne!(old.id(), next.id());
    armed(&next, 1);
    client.advance_to(50).unwrap();
    let (m, r) = packet(7, vec![70]);
    client.send(next.id(), m);
    assert_eq!(done(&next, 1).result, 70);
    frozen_application(&stopped, &old.snapshot());
    released(&[r], 8);
    rt.shutdown();

    // Expiry bursts are serviced while application turns still happen.
    let rt = Runtime::new(allowance);
    let client = rt.client();
    let group: Vec<_> = (0..19)
        .map(|_| rt.spawn_input(timed::program(), input(7, 5)).unwrap())
        .collect();
    for h in &group {
        armed(h, 1);
    }
    assert_eq!(client.clock_snapshot().pending, 19);
    client.advance_to(5).unwrap();
    for h in &group {
        assert_eq!(done(h, 1).result, 999);
    }
    let c = client.clock_snapshot();
    assert_eq!((c.expired, c.pending, c.service_batches), (19, 0, 3));
    assert!(c.polls_with_due > 0);
    rt.shutdown();

    // No expiry for infinity, and shutdown owns finite timers plus mailbox data.
    let rt = Runtime::new(allowance);
    let client = rt.client();
    let infinite = rt.spawn(waiter::program(), vec![]).unwrap();
    wait(&infinite, |s| s.waiting);
    client.advance_to(100).unwrap();
    done(&rt.spawn(peer::program(), vec![]).unwrap(), 1);
    assert_eq!(
        (infinite.snapshot().timeouts, client.clock_snapshot().armed),
        (0, 0)
    );
    let finite = rt.spawn_input(timed::program(), input(7, 50)).unwrap();
    armed(&finite, 1);
    let (m, r) = packet(1, vec![10]);
    client.send(finite.id(), m);
    wait(&finite, |s| s.waiting && s.scans == 1);
    rt.shutdown();
    done(&infinite, 2);
    done(&finite, 2);
    released(&[r], 8);
    assert_eq!(client.clock_snapshot().pending, 0);
    assert_eq!(client.advance_to(200), Err(ClockError::Closed));

    // Actual failed polls release held input and queued messages without poisoning locks.
    for overflow in [false, true] {
        let rt = Runtime::paused(allowance);
        let client = rt.client();
        if overflow {
            client.advance_to(u128::MAX).unwrap();
        }
        let h = rt
            .spawn_input(
                timed::program(),
                input(7, if overflow { 1 } else { u64::MAX }),
            )
            .unwrap();
        let (m, r) = packet(1, vec![10]);
        client.send(h.id(), m);
        rt.start();
        assert_eq!((done(&h, 3).drops, h.snapshot().cleanup_messages), (1, 1));
        released(&[r], 8);
        assert_eq!(client.clock_snapshot().pending, 0);
        done(&rt.spawn(peer::program(), vec![]).unwrap(), 1);
        rt.shutdown();
    }

    let rt = Runtime::new(allowance);
    let client = rt.client();
    for tick in 1..=16_u128 {
        let h = rt.spawn_input(timed::program(), input(7, 1)).unwrap();
        armed(&h, 1);
        let (m, r) = packet(7, vec![70]);
        let barrier = Arc::new(Barrier::new(3));
        std::thread::scope(|scope| {
            let barrier = &barrier;
            let client = &client;
            let id = h.id();
            scope.spawn(move || {
                barrier.wait();
                client.send(id, m);
            });
            scope.spawn(move || {
                barrier.wait();
                client.advance_to(tick).unwrap();
            });
            barrier.wait();
        });
        let s = done(&h, 1);
        assert!((s.result == 70 && s.timeouts == 0) || (s.result == 999 && s.timeouts == 1));
        released(&[r], 8);
        assert_eq!(client.clock_snapshot().pending, 0);
    }
    for tick in 17..=32_u128 {
        let h = rt.spawn_input(timed::program(), input(7, 1)).unwrap();
        armed(&h, 1);
        let barrier = Arc::new(Barrier::new(3));
        let receipt = std::thread::scope(|scope| {
            let barrier = &barrier;
            let client = &client;
            let target = &h;
            let kill = scope.spawn(move || {
                barrier.wait();
                target.kill()
            });
            scope.spawn(move || {
                barrier.wait();
                client.advance_to(tick).unwrap();
            });
            barrier.wait();
            kill.join().unwrap()
        });
        let s = wait(&h, |s| s.cleanup_event > 0);
        assert_eq!(
            s.terminal,
            if receipt == KillReceipt::Accepted {
                2
            } else {
                1
            }
        );
        if s.terminal == 1 {
            assert_eq!(s.result, 999);
        }
        assert_eq!((s.drops, client.clock_snapshot().pending), (1, 0));
        done(&rt.spawn(peer::program(), vec![]).unwrap(), 1);
        frozen_application(&s, &h.snapshot());
    }
    rt.shutdown();
    println!("{{\"kind\":\"timeout_controls\",\"passed\":true,\"allowance\":{allowance},\"zero_scan_candidates\":129,\"burst_timers\":19,\"burst_service_batches\":3,\"timer_batch_limit\":8,\"application_turn_during_due_work\":true,\"send_timeout_races\":16,\"kill_timeout_races\":16,\"held_result_cleanup\":true,\"shutdown_timer_cleanup\":true,\"unmatched_no_rearm\":true,\"operands_evaluated_once\":true,\"nested_match_budget_equivalent\":true,\"speed_tests\":false}}");
}
