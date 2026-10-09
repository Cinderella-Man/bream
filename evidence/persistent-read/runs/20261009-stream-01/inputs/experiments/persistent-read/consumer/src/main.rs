use bream_stream_macros::process_module;
use bream_stream_runtime::{
    Envelope, Input, KillReceipt, MessageReceipt, ProcessHandle, Runtime, Snapshot,
};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};
mod busy;
mod reads;
mod streams;
mod exits;
mod replacement;
mod links;
mod monitors;
mod room;
mod shared;
mod spawns;
mod timeouts;

#[process_module]
mod ordered {
    fn take(tag: u64) -> Words {
        let message = receive(tag);
        return message_words(message);
    }
    fn run() {
        let first = take(2);
        let second = take(1);
        let third = message_words(receive_any());
        let fourth = message_words(receive_any());
        result(
            read(first, 0) * 1000000
                + read(second, 0) * 10000
                + read(third, 0) * 100
                + read(fourth, 0),
        );
    }
}
#[process_module]
mod waiter {
    fn run() {
        mark(1);
        let message = receive(9999);
        result(message_tag(message));
        mark(2);
    }
}
#[process_module]
mod held {
    struct State {
        packet: Message,
        input: Words,
    }
    fn pause(state: State) {
        mark(70);
        let next = receive(99);
        let payload = message_words(state.packet);
        result(read(payload, 0) + message_tag(next) + len(state.input));
    }
    fn run() {
        let state = State {
            packet: receive(7),
            input: owned_input(),
        };
        pause(state);
    }
}
#[process_module]
mod self_echo {
    fn run() {
        let me = self_id();
        send(me, 7, owned_input());
        let words = message_words(receive(7));
        result(read(words, 0) + read(words, 1));
    }
}
#[process_module]
mod self_wait {
    fn run() {
        send(self_id(), 7, owned_input());
        let message = receive(8);
        result(message_tag(message));
    }
}
#[process_module]
mod sender {
    fn run() {
        send(input_target(), input_tag(), owned_input());
        mark(7);
    }
}
#[process_module]
mod gated_sink {
    fn run() {
        let gate = receive(100);
        let words = message_words(receive(7));
        result(read(words, 0) + message_tag(gate));
    }
}
#[process_module]
mod forwarder {
    fn destination() -> Pid {
        return input_target();
    }
    fn run() {
        for _count in 0..3 {
            let message = receive_any();
            send(destination(), message_tag(message), message_words(message));
        }
    }
}
#[process_module]
mod collector {
    fn run() {
        let mut sum: u64 = 0;
        for _count in 0..3 {
            let words = message_words(receive_any());
            sum = sum * 10 + read(words, 0);
        }
        result(sum);
    }
}
#[process_module]
mod concurrent_order {
    fn run() {
        let mut first: u64 = 0;
        let mut second: u64 = 0;
        for _count in 0..16 {
            let words = message_words(receive_any());
            if read(words, 0) == 1 {
                if read(words, 1) != first {
                    result(999);
                    return;
                }
                first += 1;
            } else {
                if read(words, 1) != second {
                    result(999);
                    return;
                }
                second += 1;
            }
        }
        result(first + second);
    }
}
#[process_module]
mod scan_target {
    fn run() {
        let words = message_words(receive(99));
        result(read(words, 0));
    }
}
#[process_module]
mod peer {
    fn run() {
        result(9);
    }
}
#[process_module]
mod active_peer {
    fn run() {
        for step in 0..32 {
            observe(step + 1);
        }
    }
}

// Watchdog only: elapsed time is not retained, reported, or compared.
fn wait(handle: &ProcessHandle, condition: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let snapshot = handle.snapshot();
        if condition(&snapshot) {
            return snapshot;
        }
        assert!(Instant::now() < deadline, "correctness watchdog expired");
        std::thread::yield_now();
    }
}
fn done(handle: &ProcessHandle, terminal: u8) -> Snapshot {
    let s = wait(handle, |s| s.cleanup_event > 0);
    assert_eq!(s.terminal, terminal);
    assert!(s.stop_event > 0 && s.cleanup_event > s.stop_event && !s.polling && !s.waiting);
    assert_eq!(s.queued, 0);
    assert_eq!(
        (s.timer_generation, s.timer_deadline, s.timer_expired),
        (None, None, false)
    );
    s
}
fn packet(tag: u64, data: Vec<u64>) -> (Envelope, MessageReceipt) {
    Envelope::new(tag, data).unwrap()
}
fn released(receipts: &[MessageReceipt], bytes: u64) {
    for r in receipts {
        assert_eq!(r.drops(), 1);
        assert_eq!((r.live_bytes(), r.freed_bytes()), (0, bytes));
    }
}
fn frozen_application(a: &Snapshot, b: &Snapshot) {
    assert_eq!(
        (
            a.polls,
            a.steps,
            a.progress,
            a.stage,
            a.result,
            a.scans,
            a.receives,
            a.timeouts,
            a.terminal,
            a.stop_event
        ),
        (
            b.polls,
            b.steps,
            b.progress,
            b.stage,
            b.result,
            b.scans,
            b.receives,
            b.timeouts,
            b.terminal,
            b.stop_event
        )
    );
}
fn controls(allowance: u32) {
    assert!(
        matches!(allowance, 1 | 4 | 32),
        "logical scheduling control uses allowances 1, 4 or 32"
    );
    let rt = Runtime::paused(allowance);
    let client = rt.client();
    let target = rt.spawn(ordered::program(), vec![0]).unwrap();
    let mut receipts = Vec::new();
    for (tag, value) in [(1, 10), (2, 20), (1, 30), (3, 40)] {
        let (message, receipt) = packet(tag, vec![value]);
        client.send(target.id(), message);
        receipts.push(receipt);
    }
    rt.start();
    let s = done(&target, 1);
    assert_eq!((s.result, s.receives, s.scans), (20103040, 4, 5));
    released(&receipts, 8);
    rt.shutdown();

    let rt = Runtime::new(allowance);
    let client = rt.client();
    let target = rt.spawn(waiter::program(), vec![1]).unwrap();
    let parked = wait(&target, |s| s.waiting && s.parks > 0);
    let peer = rt.spawn(active_peer::program(), vec![]).unwrap();
    done(&peer, 1);
    assert_eq!(target.snapshot().polls, parked.polls); // A parked process is not busy-polled.
    let (unmatched, receipt) = packet(1, vec![10]);
    client.send(target.id(), unmatched);
    wait(&target, |s| s.waiting && s.scans == 1);
    let (another, receipt2) = packet(2, vec![20]);
    client.send(target.id(), another);
    wait(&target, |s| s.waiting && s.scans == 2);
    assert_eq!(target.kill(), KillReceipt::Accepted);
    let stopped = done(&target, 2);
    assert_eq!(
        (stopped.stage, stopped.receives, stopped.cleanup_messages),
        (1, 0, 2)
    );
    released(&[receipt, receipt2], 8);
    let peer = rt.spawn(peer::program(), vec![]).unwrap();
    done(&peer, 1);
    frozen_application(&stopped, &target.snapshot());
    rt.shutdown();

    let rt = Runtime::new(allowance);
    let client = rt.client();
    let target = rt.spawn(held::program(), vec![11]).unwrap();
    let (selected, selected_receipt) = packet(7, vec![70]);
    client.send(target.id(), selected);
    wait(&target, |s| s.stage == 70 && s.waiting);
    let (queued, queued_receipt) = packet(8, vec![80]);
    client.send(target.id(), queued);
    wait(&target, |s| s.waiting && s.scanned_prefix == 1);
    assert_eq!((selected_receipt.drops(), queued_receipt.drops()), (0, 0));
    target.kill();
    let s = done(&target, 2);
    assert_eq!(
        (s.drops, s.freed_bytes, s.receives, s.cleanup_messages),
        (1, 8, 1, 1)
    );
    released(&[selected_receipt, queued_receipt], 8);
    rt.shutdown();

    let rt = Runtime::new(allowance);
    let echo = rt.spawn(self_echo::program(), vec![4, 5]).unwrap();
    let s = done(&echo, 1);
    assert_eq!((s.result, s.drops, s.freed_bytes), (9, 1, 16));
    let blocked = rt.spawn(self_wait::program(), vec![1, 2]).unwrap();
    wait(&blocked, |s| s.waiting && s.scanned_prefix == 1);
    blocked.kill();
    assert_eq!(
        (done(&blocked, 2).drops, blocked.snapshot().freed_bytes),
        (1, 16)
    );
    rt.shutdown();

    let rt = Runtime::new(allowance);
    let client = rt.client();
    let sink = rt.spawn(gated_sink::program(), vec![]).unwrap();
    let source = rt
        .spawn_to(
            sender::program(),
            Input {
                words: vec![42],
                tag: 7,
                argument: 0,
            },
            sink.id(),
        )
        .unwrap();
    let source_done = done(&source, 1);
    wait(&sink, |s| s.waiting && s.scanned_prefix == 1);
    assert_eq!((source_done.live_bytes, source_done.drops), (8, 0));
    let (gate, gate_receipt) = packet(100, vec![]);
    client.send(sink.id(), gate);
    assert_eq!(done(&sink, 1).result, 142);
    assert_eq!(
        (source.snapshot().live_bytes, source.snapshot().drops),
        (0, 1)
    );
    frozen_application(&source_done, &source.snapshot());
    released(&[gate_receipt], 0);
    rt.shutdown();

    let rt = Runtime::new(allowance);
    let client = rt.client();
    let sink = rt.spawn(collector::program(), vec![]).unwrap();
    let relay = rt
        .spawn_to(
            forwarder::program(),
            Input {
                words: vec![],
                tag: 0,
                argument: 0,
            },
            sink.id(),
        )
        .unwrap();
    let mut receipts = Vec::new();
    for value in 1..=3 {
        let (message, receipt) = packet(7, vec![value]);
        client.send(relay.id(), message);
        receipts.push(receipt);
    }
    done(&relay, 1);
    assert_eq!(done(&sink, 1).result, 123);
    released(&receipts, 8);
    rt.shutdown();

    let rt = Runtime::paused(allowance);
    let client = rt.client();
    let scanner = rt.spawn(scan_target::program(), vec![]).unwrap();
    let peer = rt.spawn(peer::program(), vec![]).unwrap();
    let mut receipts = Vec::new();
    for _ in 0..128 {
        let (message, receipt) = packet(1, vec![10]);
        client.send(scanner.id(), message);
        receipts.push(receipt);
    }
    let (matching, matched_receipt) = packet(99, vec![123]);
    client.send(scanner.id(), matching);
    rt.start();
    let p = done(&peer, 1);
    let s = done(&scanner, 1);
    assert_eq!((s.result, s.scans, s.cleanup_messages), (123, 129, 128));
    assert!(p.stop_event < s.stop_event); // Logical peer progress during scan, not a timing claim.
    released(&receipts, 8);
    released(&[matched_receipt], 8);
    rt.shutdown();

    let rt = Runtime::paused(allowance);
    let client = rt.client();
    let victim = rt.spawn(waiter::program(), vec![]).unwrap();
    let peer = rt.spawn(peer::program(), vec![]).unwrap();
    let mut receipts = Vec::new();
    for _ in 0..65 {
        let (message, receipt) = packet(1, vec![10]);
        client.send(victim.id(), message);
        receipts.push(receipt);
    }
    victim.kill();
    rt.start();
    let p = done(&peer, 1);
    let v = done(&victim, 2);
    assert_eq!((v.polls, v.cleanup_batches, v.cleanup_messages), (0, 9, 65));
    assert!(v.stop_event < p.stop_event && p.stop_event < v.cleanup_event);
    released(&receipts, 8);
    rt.shutdown();

    let rt = Runtime::with_capacity(allowance, 1);
    let client = rt.client();
    let old = rt.spawn(waiter::program(), vec![]).unwrap();
    wait(&old, |s| s.waiting);
    old.kill();
    done(&old, 2);
    let replacement = rt.spawn(waiter::program(), vec![]).unwrap();
    wait(&replacement, |s| s.waiting);
    assert_eq!(old.id().slot(), replacement.id().slot());
    assert_ne!(old.id(), replacement.id());
    let (message, receipt) = packet(9999, vec![10]);
    client.send(old.id(), message);
    released(&[receipt], 8);
    assert_eq!(replacement.snapshot().queued, 0);
    let foreign = Runtime::new(allowance);
    let (message, receipt) = packet(9999, vec![10]);
    foreign.client().send(replacement.id(), message);
    released(&[receipt], 8);
    foreign.shutdown();
    rt.shutdown();
    assert_eq!(done(&replacement, 2).receives, 0);

    let rt = Runtime::new(allowance);
    let client = rt.client();
    let target = rt.spawn(concurrent_order::program(), vec![]).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let senders: Vec<_> = (1..=2)
        .map(|sender| {
            let client = client.clone();
            let barrier = barrier.clone();
            let id = target.id();
            std::thread::spawn(move || {
                barrier.wait();
                let mut receipts = Vec::new();
                for value in 0..8 {
                    let (message, receipt) = packet(7, vec![sender, value]);
                    client.send(id, message);
                    receipts.push(receipt);
                }
                receipts
            })
        })
        .collect();
    barrier.wait();
    let receipts: Vec<_> = senders
        .into_iter()
        .flat_map(|s| s.join().unwrap())
        .collect();
    assert_eq!(done(&target, 1).result, 16);
    released(&receipts, 16);
    rt.shutdown();

    for _ in 0..16 {
        let rt = Runtime::new(allowance);
        let client = rt.client();
        let target = rt.spawn(waiter::program(), vec![]).unwrap();
        wait(&target, |s| s.waiting);
        let (message, receipt) = packet(7, vec![70]);
        let id = target.id();
        let barrier = Arc::new(Barrier::new(2));
        let ready = barrier.clone();
        let sender = std::thread::spawn(move || {
            ready.wait();
            client.send(id, message);
        });
        barrier.wait();
        target.kill();
        sender.join().unwrap();
        done(&target, 2);
        released(&[receipt], 8);
        rt.shutdown();
    }

    let rt = Runtime::new(allowance);
    let client = rt.client();
    let target = rt.spawn(waiter::program(), vec![]).unwrap();
    let mut receipts = Vec::new();
    for _ in 0..33 {
        let (message, receipt) = packet(7, vec![70]);
        client.send(target.id(), message);
        receipts.push(receipt);
    }
    wait(&target, |s| s.waiting && s.scanned_prefix == 33);
    target.kill();
    let done = done(&target, 2); // No ready application task is needed to drain cleanup.
    assert_eq!((done.cleanup_batches, done.cleanup_messages), (5, 33));
    released(&receipts, 8);
    rt.shutdown();

    println!("{{\"kind\":\"mailbox_controls\",\"passed\":true,\"allowance\":{allowance},\"ordered_digest\":20103040,\"scan_candidates\":129,\"scan_peer_progress\":true,\"cleanup_peer_progress\":true,\"cleanup_batch_limit\":8,\"send_kill_races\":16,\"concurrent_messages\":16,\"transfer_outlives_sender\":true,\"held_and_queued_cleanup\":true,\"stale_send_isolated\":true,\"parked_no_spin\":true,\"speed_tests\":false}}");
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let allowance = args.get(2).expect("allowance").parse().unwrap();
    match args.get(1).map(String::as_str) {
        Some("controls") => controls(allowance),
        Some("timeouts") => timeouts::run(allowance),
        Some("monitors") => monitors::run(allowance),
        Some("links") => links::run(allowance),
        Some("spawns") => spawns::run(allowance),
        Some("room") => room::run(allowance),
        Some("shared") => shared::run(allowance),
        Some("busy") => busy::run(allowance),
        Some("replacement") => replacement::run(allowance),
        Some("exits") => exits::run(allowance),
        Some("reads") => reads::run(allowance),
        Some("streams") => streams::run(allowance),
        _ => panic!("expected controls, timeouts, monitors, links, spawns, room, shared, busy or replacement"),
    }
}
