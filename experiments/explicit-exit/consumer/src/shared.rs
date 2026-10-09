use super::*;
use bream_exit_runtime::{SharedBytes, SharedEnvelope, SharedReceipt};

#[process_module]
mod broadcast {
    fn checksum(bytes: SharedBytes) -> u64 {
        let mut digest: u64 = 0;
        for index in 0..shared_len(bytes) {
            digest = digest * 31 + shared_read(bytes, index);
        }
        return digest;
    }
    fn session() {
        let mut ack = owned_input();
        let count = read(ack, 0);
        if input_argument() > count {
            return;
        }
        let bytes = shared_bytes(receive_shared(7));
        if input_tag() == 1 && input_argument() == 1 {
            // Contained receiver failure while it owns an immutable reference.
            result(shared_read(bytes, shared_len(bytes)));
        }
        if input_tag() == 2 {
            match receive_for(900, 5) {
                ReceiveResult::Message(unexpected) => {
                    result(999);
                    return;
                }
                ReceiveResult::Timeout => {}
            }
        }
        let value = checksum(bytes);
        write(ack, 0, value + input_argument());
        send(input_target(), 30 + input_argument(), ack);
    }
    fn fanout(bytes: SharedBytes, first: Pid, second: Pid, count: u64) {
        if count > 0 {
            send_shared(first, 7, clone_shared(bytes));
        }
        if input_tag() == 2 {
            mark(71);
            let gate = receive(99);
        }
        if count > 1 {
            send_shared(second, 7, clone_shared(bytes));
        }
    }
    fn run() {
        let count = input_argument();
        let first = spawn_monitor(session, owned_input(), input_tag(), 1);
        let second = spawn_monitor(session, message_words(receive(2)), input_tag(), 2);
        let bytes = shared_bytes(receive_shared(7));
        fanout(bytes, spawned_pid(first), spawned_pid(second), count);
        let mut downs: u64 = 0;
        let mut acks: u64 = 0;
        let mut digest: u64 = 0;
        let mut first_done = false;
        let mut second_done = false;
        let mut first_ack = false;
        let mut second_ack = false;
        let mut expected = count;
        if input_tag() == 1 {
            expected = expected - 1;
        }
        while downs < 2 || acks < expected {
            match receive_event() {
                Event::Message(message) => {
                    let tag = message_tag(message);
                    if tag == 31 && !first_ack {
                        first_ack = true;
                    } else if tag == 32 && !second_ack {
                        second_ack = true;
                    } else {
                        result(999);
                        return;
                    }
                    let ack = message_words(message);
                    digest += read(ack, 0);
                    acks += 1;
                }
                Event::Down(down) => {
                    if down_ref(down) == spawned_ref(first)
                        && down_target(down) == spawned_pid(first)
                        && !first_done
                    {
                        if count > 0 && input_tag() != 1 && !first_ack {
                            result(999);
                            return;
                        }
                        first_done = true;
                        let mut reason: u64 = 1;
                        if input_tag() == 1 {
                            reason = 3;
                        }
                        if down_reason(down) != reason {
                            result(999);
                            return;
                        }
                    } else if down_ref(down) == spawned_ref(second)
                        && down_target(down) == spawned_pid(second)
                        && !second_done
                    {
                        if count > 1 && !second_ack {
                            result(999);
                            return;
                        }
                        second_done = true;
                        if down_reason(down) != 1 {
                            result(999);
                            return;
                        }
                    } else {
                        result(999);
                        return;
                    }
                    downs += 1;
                }
                Event::Shared(extra) => {
                    result(999);
                    return;
                }
                Event::Exit(extra_exit) => {
                    result(999);
                    return;
                }
            }
        }
        observe(acks);
        result(digest);
    }
}

#[process_module]
mod reader {
    struct Held {
        bytes: SharedBytes,
    }
    fn finish(held: Held) {
        mark(70);
        let gate = receive(99);
        let mut digest: u64 = 0;
        for index in 0..shared_len(held.bytes) {
            digest = digest * 31 + shared_read(held.bytes, index);
        }
        result(digest);
    }
    fn run() {
        let held = Held {
            bytes: shared_bytes(receive_shared(7)),
        };
        finish(held);
    }
}

#[process_module]
mod publisher {
    fn run() {
        let bytes = shared_bytes(receive_shared_any());
        send_shared(input_target(), 7, clone_shared(bytes));
        mark(80);
        let gate = receive(99);
        result(shared_read(bytes, 0));
    }
}

#[process_module]
mod timed_reader {
    fn run() {
        match receive_shared_for(9, 5) {
            SharedReceiveResult::Message(unexpected) => {
                result(999);
                return;
            }
            SharedReceiveResult::Timeout => {}
        }
        let bytes = shared_bytes(receive_shared_any());
        result(shared_read(bytes, 0));
    }
}

#[process_module]
mod pending_sender {
    fn hold(bytes: SharedBytes) -> SharedBytes {
        mark(90);
        let gate = receive(99);
        return bytes;
    }
    fn run() {
        let original = shared_bytes(receive_shared_any());
        send_shared(input_target(), 7, hold(clone_shared(original)));
        result(shared_read(original, 0));
    }
}

fn payload() -> (SharedBytes, SharedReceipt) {
    SharedBytes::new(vec![2, 3, 5, 7]).unwrap()
}
fn freed(receipt: &SharedReceipt, references: u64) {
    let s = receipt.snapshot();
    assert_eq!((s.live_bytes, s.freed_bytes, s.physical_drops), (0, 4, 1));
    assert_eq!(
        (
            s.live_references,
            s.references_created,
            s.references_dropped
        ),
        (0, references, references)
    );
}
fn gate(client: &bream_exit_runtime::RuntimeClient, target: &ProcessHandle) -> MessageReceipt {
    let (message, receipt) = packet(99, vec![]);
    client.send(target.id(), message);
    receipt
}
fn quiescent(client: &bream_exit_runtime::RuntimeClient, live: usize) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while client.live_count() != live {
        assert!(Instant::now() < deadline, "correctness watchdog");
        std::thread::yield_now();
    }
}

pub(super) fn run(allowance: u32) {
    let checksum = [2_u64, 3, 5, 7]
        .into_iter()
        .fold(0, |sum, byte| sum * 31 + byte);
    let mut observed_broadcast = Vec::new();
    for (count, fault) in [(0, false), (1, false), (2, false), (2, true)] {
        let runtime = Runtime::paused(allowance);
        let client = runtime.client();
        let room = client
            .spawn_input(
                broadcast::program(),
                Input {
                    words: vec![count],
                    tag: u64::from(fault),
                    argument: count,
                },
            )
            .unwrap();
        let (startup, startup_receipt) = packet(2, vec![count]);
        client.send(room.id(), startup);
        let (bytes, receipt) = payload();
        client.send_shared(room.id(), SharedEnvelope::new(7, bytes));
        runtime.start();
        let s = done(&room, 1);
        let expected = if fault {
            checksum + 2
        } else {
            checksum * count + count * (count + 1) / 2
        };
        assert_eq!((s.result, s.progress), (expected, count - u64::from(fault)));
        quiescent(&client, 0);
        assert_eq!(client.active_monitors(), 0);
        runtime.shutdown();
        freed(&receipt, count + 1);
        released(&[startup_receipt], 8);
        observed_broadcast.push([count, u64::from(fault), s.result, s.progress]);
    }

    // A real publisher sends a clone, then dies while retaining its original.
    let runtime = Runtime::new(allowance);
    let client = runtime.client();
    let survivor = client.spawn(reader::program(), vec![]).unwrap();
    let sender = client
        .spawn_to(
            publisher::program(),
            Input {
                words: vec![],
                tag: 0,
                argument: 0,
            },
            survivor.id(),
        )
        .unwrap();
    let (bytes, receipt) = payload();
    client.send_shared(sender.id(), SharedEnvelope::new(1, bytes));
    wait(&sender, |s| s.stage == 80 && s.waiting);
    wait(&survivor, |s| s.stage == 70 && s.waiting);
    assert_eq!(receipt.snapshot().live_references, 2);
    assert_eq!(sender.kill(), KillReceipt::Accepted);
    done(&sender, 2);
    assert_eq!(
        (
            receipt.snapshot().live_references,
            receipt.snapshot().physical_drops
        ),
        (1, 0)
    );
    let gate_receipt = gate(&client, &survivor);
    let after_publisher = done(&survivor, 1).result;
    assert_eq!(after_publisher, checksum);
    runtime.shutdown();
    freed(&receipt, 2);
    released(&[gate_receipt], 0);

    // Killing a reader leaves a second reader and a host reference intact.
    let runtime = Runtime::new(allowance);
    let client = runtime.client();
    let first = client.spawn(reader::program(), vec![]).unwrap();
    let second = client.spawn(reader::program(), vec![]).unwrap();
    let (host, receipt) = payload();
    for target in [&first, &second] {
        client.send_shared(target.id(), SharedEnvelope::new(7, host.clone()));
        wait(target, |s| s.stage == 70 && s.waiting);
    }
    assert_eq!(first.kill(), KillReceipt::Accepted);
    done(&first, 2);
    assert_eq!(
        (
            receipt.snapshot().live_references,
            receipt.snapshot().physical_drops
        ),
        (2, 0)
    );
    let gate_receipt = gate(&client, &second);
    let after_receiver = done(&second, 1).result;
    assert_eq!(after_receiver, checksum);
    runtime.shutdown();
    assert_eq!((receipt.snapshot().live_references, host.read(3)), (1, 7));
    drop(host);
    freed(&receipt, 3);
    released(&[gate_receipt], 0);

    // Stop exactly between two fanout sends. Published shares survive it;
    // the second share must never be created. Logical time releases reader 1.
    let runtime = Runtime::new(allowance);
    let client = runtime.client();
    let room = client
        .spawn_input(
            broadcast::program(),
            Input {
                words: vec![2],
                tag: 2,
                argument: 2,
            },
        )
        .unwrap();
    let (startup, startup_receipt) = packet(2, vec![2]);
    client.send(room.id(), startup);
    let (bytes, receipt) = payload();
    client.send_shared(room.id(), SharedEnvelope::new(7, bytes));
    wait(&room, |s| s.stage == 71 && s.waiting);
    let deadline = Instant::now() + Duration::from_secs(30);
    while client.clock_snapshot().pending != 1 {
        assert!(Instant::now() < deadline, "correctness watchdog");
        std::thread::yield_now();
    }
    assert_eq!(room.kill(), KillReceipt::Accepted);
    done(&room, 2);
    assert_eq!(
        (
            receipt.snapshot().live_references,
            receipt.snapshot().references_created
        ),
        (1, 2)
    );
    client.advance_to(5).unwrap();
    quiescent(&client, 1); // Only the never-addressed second session remains.
    runtime.shutdown();
    freed(&receipt, 2);
    released(&[startup_receipt], 8);

    // Shared receive uses the same deadline-preserving selector as Words.
    let runtime = Runtime::new(allowance);
    let client = runtime.client();
    let timer = client.spawn(timed_reader::program(), vec![]).unwrap();
    let (bytes, receipt) = payload();
    client.send_shared(timer.id(), SharedEnvelope::new(7, bytes));
    wait(&timer, |s| {
        s.waiting && s.timer_deadline == Some(5) && s.scanned_prefix == 1
    });
    client.advance_to(5).unwrap();
    let s = done(&timer, 1);
    assert_eq!((s.result, s.timeouts), (2, 1));
    runtime.shutdown();
    freed(&receipt, 1);

    // Held and queued references are retired on host shutdown, in batches.
    let runtime = Runtime::new(allowance);
    let client = runtime.client();
    let target = client.spawn(reader::program(), vec![]).unwrap();
    let (host, receipt) = payload();
    client.send_shared(target.id(), SharedEnvelope::new(7, host.clone()));
    wait(&target, |s| s.stage == 70 && s.waiting);
    for tag in 0..19 {
        client.send_shared(target.id(), SharedEnvelope::new(tag, host.clone()));
    }
    wait(&target, |s| s.waiting && s.scanned_prefix == 19);
    runtime.shutdown();
    let s = done(&target, 2);
    assert_eq!((s.cleanup_messages, s.cleanup_batches), (19, 3));
    assert_eq!(
        (
            receipt.snapshot().live_references,
            receipt.snapshot().physical_drops
        ),
        (1, 0)
    );
    drop(host);
    freed(&receipt, 21);
    assert_eq!(
        (
            client.live_count(),
            client.active_monitors(),
            client.clock_snapshot().pending
        ),
        (0, 0, 0)
    );

    for _ in 0..16 {
        let runtime = Runtime::new(allowance);
        let client = runtime.client();
        let target = client.spawn(reader::program(), vec![]).unwrap();
        wait(&target, |s| s.waiting);
        let (host, receipt) = payload();
        let sent = host.clone();
        let barrier = Arc::new(Barrier::new(2));
        let sender_barrier = barrier.clone();
        let sender_client = client.clone();
        let id = target.id();
        let sender = std::thread::spawn(move || {
            sender_barrier.wait();
            sender_client.send_shared(id, SharedEnvelope::new(7, sent));
        });
        barrier.wait();
        assert_eq!(target.kill(), KillReceipt::Accepted);
        sender.join().unwrap();
        done(&target, 2);
        runtime.shutdown();
        assert_eq!(
            (
                receipt.snapshot().live_references,
                receipt.snapshot().physical_drops,
                host.read(2)
            ),
            (1, 0, 5)
        );
        drop(host);
        freed(&receipt, 2);
    }

    // Kill during evaluation of a cloned send operand, before publication.
    let runtime = Runtime::new(allowance);
    let client = runtime.client();
    let target = client.spawn(reader::program(), vec![]).unwrap();
    let sender = client
        .spawn_to(
            pending_sender::program(),
            Input {
                words: vec![],
                tag: 0,
                argument: 0,
            },
            target.id(),
        )
        .unwrap();
    let (bytes, receipt) = payload();
    client.send_shared(sender.id(), SharedEnvelope::new(7, bytes));
    wait(&sender, |s| s.stage == 90 && s.waiting);
    wait(&target, |s| s.waiting);
    assert_eq!(receipt.snapshot().live_references, 2);
    assert_eq!(sender.kill(), KillReceipt::Accepted);
    done(&sender, 2);
    freed(&receipt, 2);
    assert_eq!(
        (target.snapshot().queued, target.snapshot().receives),
        (0, 0)
    );
    runtime.shutdown();
    done(&target, 2);

    println!("{{\"kind\":\"shared_controls\",\"passed\":true,\"allowance\":{allowance},\"broadcast_observations\":{observed_broadcast:?},\"after_publisher_checksum\":{after_publisher},\"after_receiver_checksum\":{after_receiver},\"partial_fanout_cancelled\":true,\"unpublished_operand_cancelled\":true,\"shared_deadline_preserves_nonmatch\":true,\"host_reference_survives_shutdown\":true,\"cleanup_messages\":{},\"cleanup_batches\":{},\"shared_send_kill_races\":16,\"physical_release_once\":true,\"speed_tests\":false}}", s.cleanup_messages, s.cleanup_batches);
}
