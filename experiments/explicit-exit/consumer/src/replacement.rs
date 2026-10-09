use super::*;
use bream_exit_runtime::{
    RuntimeClient, SharedBytes, SharedEnvelope, SharedReceipt, SharedSnapshot,
};

#[process_module]
mod application {
    struct Slot {
        child: Spawned,
        epoch: u64,
    }
    fn answer(bytes: SharedBytes, mut words: Words, tag: u64) -> SharedBytes {
        let mut digest: u64 = 0;
        for index in 0..shared_len(bytes) {
            digest = digest * 31 + shared_read(bytes, index);
        }
        write(words, 0, input_tag());
        write(words, 2, digest);
        send(input_target(), tag, words);
        return bytes;
    }
    fn member() {
        let room = input_target();
        let watching_room = monitor(room);
        let mut bytes = shared_bytes(receive_shared(7));
        bytes = answer(bytes, owned_input(), 200 + input_tag());
        while true {
            match receive_event() {
                Event::Message(message) => {
                    let tag = message_tag(message);
                    if tag == 100 {
                        bytes = answer(bytes, message_words(message), 100 + input_tag());
                    } else if tag == 300 && input_tag() == 1 {
                        // A real old-incarnation report is queued before its Down.
                        bytes = answer(bytes, message_words(message), 301);
                        if input_argument() == 1 {
                            result(shared_read(bytes, shared_len(bytes)));
                        }
                        result(shared_len(bytes));
                        return;
                    } else if tag == 900 {
                        send(room, 900 + input_tag(), message_words(message));
                        result(shared_len(bytes));
                        return;
                    } else {
                        result(999);
                        return;
                    }
                }
                Event::Down(down) => {
                    if down_ref(down) == watching_room && down_target(down) == room {
                        result(shared_len(bytes));
                        return;
                    }
                    // Wrong notifications cannot mimic successful room teardown.
                    result(999);
                }
                Event::Shared(unexpected_shared) => {
                    result(999);
                    return;
                }
                Event::Exit(unexpected_exit) => {
                    result(999);
                    return;
                }
            }
        }
    }
    fn verified(message: Message, epoch: u64, sequence: u64) -> u64 {
        let words = message_words(message);
        if read(words, 0) != epoch || read(words, 1) != sequence || read(words, 2) != 62627 {
            return 999;
        }
        return read(words, 2) * 10 + read(words, 1);
    }
    fn run() {
        let mut slot = Slot {
            child: spawn_monitor(member, owned_input(), 1, input_argument()),
            epoch: 1,
        };
        let survivor = spawn_monitor(member, message_words(receive(2)), 9, 0);
        let resident = shared_bytes(receive_shared(7));
        send_shared(spawned_pid(slot.child), 7, clone_shared(resident));
        send_shared(spawned_pid(survivor), 7, clone_shared(resident));
        if verified(receive(201), 1, 0) != 626270 || verified(receive(209), 9, 0) != 626270 {
            result(999);
            return;
        }
        send(spawned_pid(survivor), 100, message_words(receive(11)));
        let before = verified(receive(109), 9, 1);
        if before != 626271 {
            result(999);
            return;
        }
        result(before);
        observe(1);
        mark(10);
        send(spawned_pid(slot.child), 300, message_words(receive(10)));
        mark(11);
        // Host does not supply this input until old reply and genuine Down are
        // already queued, plus a request bearing the retired application epoch.
        let replacement_input = message_words(receive(20));
        let retired = slot.child;
        slot = Slot {
            child: spawn_monitor(member, replacement_input, 2, 0),
            epoch: 2,
        };
        if spawned_pid(slot.child) == spawned_pid(retired)
            || spawned_ref(slot.child) == spawned_ref(retired)
        {
            result(999);
            return;
        }
        send_shared(spawned_pid(slot.child), 7, clone_shared(resident));
        if verified(receive(202), 2, 0) != 626270 {
            result(999);
            return;
        }
        // Old reference cancellation must not cancel the replacement's monitor,
        // and plain cancellation must leave its already queued Down intact.
        demonitor(spawned_ref(retired));
        send(spawned_pid(retired), 777, message_words(receive(30)));
        send_shared(spawned_pid(retired), 777, clone_shared(resident));
        mark(20);
        send(spawned_pid(slot.child), 100, message_words(receive(40)));
        let mut reports: u64 = 0;
        let mut old_downs: u64 = 0;
        let mut rejected: u64 = 0;
        let mut current_replies: u64 = 0;
        while reports + old_downs + rejected + current_replies < 4 {
            match receive_event() {
                Event::Message(message) => {
                    let tag = message_tag(message);
                    if tag == 301 && reports == 0 {
                        if verified(message, 1, 99) != 626369 {
                            result(999);
                            return;
                        }
                        reports += 1;
                    } else if tag == 50 && rejected == 0 {
                        let stale = message_words(message);
                        if read(stale, 0) == slot.epoch {
                            result(999);
                            return;
                        }
                        rejected += 1;
                    } else if tag == 102
                        && current_replies == 0
                        && reports == 1
                        && old_downs == 1
                        && rejected == 1
                    {
                        let value = verified(message, slot.epoch, 2);
                        if value != 626272 {
                            result(999);
                            return;
                        }
                        result(value);
                        current_replies += 1;
                    } else {
                        result(999);
                        return;
                    }
                }
                Event::Down(down) => {
                    let mut reason: u64 = 1;
                    if input_argument() == 1 {
                        reason = 3;
                    }
                    if down_ref(down) != spawned_ref(retired)
                        || down_target(down) != spawned_pid(retired)
                        || down_reason(down) != reason
                        || reports != 1
                        || old_downs != 0
                    {
                        result(999);
                        return;
                    }
                    // Account the retired record; current membership stays intact.
                    old_downs += 1;
                }
                Event::Shared(unexpected_shared) => {
                    result(999);
                    return;
                }
                Event::Exit(unexpected_exit) => {
                    result(999);
                    return;
                }
            }
        }
        observe(reports * 100 + old_downs * 10 + rejected);
        mark(30);
        send(spawned_pid(survivor), 100, message_words(receive(12)));
        let after = verified(receive(109), 9, 3);
        if after != 626273 {
            result(999);
            return;
        }
        result(after);
        observe(112);
        mark(40);
        send(spawned_pid(slot.child), 900, message_words(receive(900)));
        let current_ack = receive(902);
        let current_down = receive_down(spawned_ref(slot.child));
        if down_target(current_down) != spawned_pid(slot.child) || down_reason(current_down) != 1 {
            result(999);
            return;
        }
        send(spawned_pid(survivor), 900, message_words(receive(901)));
        let survivor_ack = receive(909);
        let survivor_down = receive_down(spawned_ref(survivor));
        if down_target(survivor_down) != spawned_pid(survivor) || down_reason(survivor_down) != 1 {
            result(999);
            return;
        }
        result(after + shared_len(resident) - 4);
        mark(50);
    }
}

fn at(handle: &ProcessHandle, stage: u64, saved: usize) -> Snapshot {
    let s = wait(handle, |s| {
        s.terminal != 0 || (s.stage == stage && s.waiting && s.scanned_prefix == saved)
    });
    assert_eq!((s.terminal, s.stage, s.scanned_prefix), (0, stage, saved));
    s
}
fn row(s: SharedSnapshot) -> [u64; 6] {
    [
        s.live_bytes,
        s.freed_bytes,
        s.physical_drops,
        s.live_references,
        s.references_created,
        s.references_dropped,
    ]
}
fn input_released(s: &Snapshot) {
    assert_eq!((s.drops, s.live_bytes, s.freed_bytes), (1, 0, 32));
}
struct Fixture {
    runtime: Runtime,
    client: RuntimeClient,
    room: ProcessHandle,
    host: SharedBytes,
    payload: SharedReceipt,
    receipts: Vec<MessageReceipt>,
    old_reply: MessageReceipt,
    stale_request: MessageReceipt,
    stale_send: MessageReceipt,
    checkpoints: Vec<[u64; 4]>,
}
impl Fixture {
    fn new(allowance: u32, failure: bool) -> Self {
        // Room plus two members fills the registry. After old exit there is
        // exactly one free slot, so successful replacement necessarily reuses it.
        let runtime = Runtime::paused_with_capacity(allowance, 3);
        let client = runtime.client();
        let room = client
            .spawn_input(
                application::program(),
                Input {
                    words: vec![0; 4],
                    tag: 0,
                    argument: u64::from(failure),
                },
            )
            .unwrap();
        let mut receipts = Vec::new();
        for (tag, values) in [(2, vec![0; 4]), (11, vec![9, 1, 0, 0])] {
            let (message, receipt) = packet(tag, values);
            client.send(room.id(), message);
            receipts.push(receipt);
        }
        let (host, payload) = SharedBytes::new(vec![2, 3, 5, 7]).unwrap();
        client.send_shared(room.id(), SharedEnvelope::new(7, host.clone()));
        runtime.start();
        let initial = at(&room, 10, 0);
        assert_eq!(
            (
                initial.progress,
                initial.result,
                client.live_count(),
                client.active_monitors()
            ),
            (1, 626271, 3, 4)
        );
        assert_eq!(row(payload.snapshot()), [4, 0, 0, 4, 4, 0]);
        input_released(&initial);
        let (message, old_reply) = packet(10, vec![1, 99, 0, 0]);
        client.send(room.id(), message);
        let old_wait = at(&room, 11, 2);
        assert_eq!(client.live_count(), 2);
        assert_eq!(client.active_monitors(), 2);
        assert_eq!(row(payload.snapshot()), [4, 0, 0, 3, 4, 1]);
        assert_eq!((old_reply.drops(), old_reply.live_bytes()), (0, 32));
        let (message, stale_request) = packet(50, vec![1, 88, 0, 0]);
        client.send(room.id(), message);
        at(&room, 11, 3);
        let (message, receipt) = packet(20, vec![0; 4]);
        client.send(room.id(), message);
        receipts.push(receipt);
        let (message, stale_send) = packet(30, vec![1, 77, 0, 0]);
        client.send(room.id(), message);
        let replaced = at(&room, 20, 3);
        assert_eq!((client.live_count(), client.active_monitors()), (3, 4));
        assert_eq!(row(payload.snapshot()), [4, 0, 0, 4, 6, 2]);
        assert_eq!((old_reply.drops(), stale_request.drops()), (0, 0));
        released(&[stale_send.clone()], 32);
        released(&receipts, 32);
        Self {
            runtime,
            client,
            room,
            host,
            payload,
            receipts,
            old_reply,
            stale_request,
            stale_send,
            checkpoints: vec![
                [
                    initial.stage,
                    initial.progress,
                    initial.result,
                    initial.queued as u64,
                ],
                [
                    old_wait.stage,
                    old_wait.progress,
                    old_wait.result,
                    old_wait.queued as u64,
                ],
                [
                    replaced.stage,
                    replaced.progress,
                    replaced.result,
                    replaced.queued as u64,
                ],
            ],
        }
    }
    fn send(&mut self, tag: u64, epoch: u64, sequence: u64) {
        let (message, receipt) = packet(tag, vec![epoch, sequence, 0, 0]);
        self.client.send(self.room.id(), message);
        self.receipts.push(receipt);
    }
    fn record(&mut self, stage: u64, progress: u64, digest: u64) {
        let s = at(&self.room, stage, 0);
        assert_eq!((s.progress, s.result), (progress, digest));
        self.checkpoints
            .push([s.stage, s.progress, s.result, s.queued as u64]);
    }
    fn queue_shares(&self) {
        for tag in 0..19 {
            self.client.send_shared(
                self.room.id(),
                SharedEnvelope::new(1000 + tag, self.host.clone()),
            );
        }
        let s = at(&self.room, 20, 22);
        assert_eq!(s.queued, 22);
        assert_eq!(row(self.payload.snapshot()), [4, 0, 0, 23, 25, 2]);
    }
}
fn release_host(host: SharedBytes, receipt: &SharedReceipt, created: u64) -> [[u64; 6]; 2] {
    assert_eq!(
        (host.read(0), host.read(1), host.read(2), host.read(3)),
        (2, 3, 5, 7)
    );
    let before = row(receipt.snapshot());
    assert_eq!(before, [4, 0, 0, 1, created, created - 1]);
    drop(host);
    let after = row(receipt.snapshot());
    assert_eq!(after, [0, 4, 1, 0, created, created]);
    [before, after]
}
fn settled(client: &RuntimeClient) {
    assert_eq!(
        (
            client.live_count(),
            client.active_monitors(),
            client.active_links(),
            client.clock_snapshot().pending
        ),
        (0, 0, 0, 0)
    );
}
pub(super) fn run(allowance: u32) {
    let mut transcripts = Vec::new();
    let mut ownership = Vec::new();
    let mut terminal_rows = Vec::new();
    for failure in [false, true] {
        let mut f = Fixture::new(allowance, failure);
        f.send(40, 2, 2);
        f.record(30, 111, 626272);
        released(&[f.old_reply.clone(), f.stale_request.clone()], 32);
        assert_eq!(row(f.payload.snapshot()), [4, 0, 0, 4, 6, 2]);
        f.send(12, 9, 3);
        f.record(40, 112, 626273);
        f.send(900, 2, 0);
        f.send(901, 9, 0);
        let completed = done(&f.room, 1);
        assert_eq!(
            (completed.stage, completed.progress, completed.result),
            (50, 112, 626273)
        );
        input_released(&completed);
        transcripts.push(f.checkpoints.clone());
        let Fixture {
            runtime,
            client,
            host,
            payload,
            mut receipts,
            old_reply,
            stale_request,
            stale_send,
            ..
        } = f;
        runtime.shutdown();
        settled(&client);
        receipts.extend([old_reply, stale_request, stale_send]);
        released(&receipts, 32);
        ownership.push(release_host(host, &payload, 6));
    }
    for explicit_kill in [false, true] {
        let f = Fixture::new(allowance, false);
        f.queue_shares();
        let Fixture {
            runtime,
            client,
            room,
            host,
            payload,
            mut receipts,
            old_reply,
            stale_request,
            stale_send,
            ..
        } = f;
        if explicit_kill {
            assert_eq!(room.kill(), KillReceipt::Accepted);
            done(&room, 2);
            let deadline = Instant::now() + Duration::from_secs(30);
            while client.live_count() != 0 {
                assert!(Instant::now() < deadline, "correctness watchdog");
                std::thread::yield_now();
            }
        }
        runtime.shutdown();
        let completed = done(&room, 2);
        assert_eq!(
            (completed.cleanup_messages, completed.cleanup_batches),
            (22, 3)
        );
        input_released(&completed);
        settled(&client);
        receipts.extend([old_reply, stale_request, stale_send]);
        released(&receipts, 32);
        ownership.push(release_host(host, &payload, 25));
        terminal_rows.push([
            u64::from(explicit_kill),
            completed.terminal as u64,
            completed.cleanup_messages,
            completed.cleanup_batches,
        ]);
    }
    println!("{{\"kind\":\"replacement_controls\",\"passed\":true,\"allowance\":{allowance},\"transcripts\":{transcripts:?},\"ownership\":{ownership:?},\"terminal_rows\":{terminal_rows:?},\"registry_capacity\":3,\"same_slot_reuse_forced\":true,\"fresh_pid_and_monitor_checked\":true,\"old_down_does_not_evict_replacement\":true,\"stale_pid_sends_discarded\":true,\"trusted_epoch_request_rejected\":true,\"speed_tests\":false}}");
}
