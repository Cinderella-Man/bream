use super::*;
use bream_stream_runtime::{
    RuntimeClient, SharedBytes, SharedEnvelope, SharedReceipt, SharedSnapshot,
};

#[process_module]
mod computation {
    struct Work {
        bytes: SharedBytes,
        scratch: Words,
        rounds: u64,
    }
    fn transform(mut work: Work) -> Work {
        let mut digest: u64 = 0;
        for index in 0..shared_len(work.bytes) {
            let byte = shared_read(work.bytes, index);
            write(work.scratch, index, byte + 1);
            digest = digest * 31 + read(work.scratch, index) - 1;
        }
        result(digest);
        work.rounds += 1;
        observe(work.rounds);
        return work;
    }
    fn burn(mut work: Work) {
        mark(70);
        // No receive, send, explicit yield, stop test or application await.
        // Every loop/helper boundary is charged by the checked compiler.
        while true {
            work = transform(work);
        }
    }
    fn run() {
        let work = Work {
            bytes: shared_bytes(receive_shared(7)),
            scratch: owned_input(),
            rounds: 0,
        };
        burn(work);
    }
}

#[process_module]
mod service {
    struct Reader {
        bytes: SharedBytes,
        startup: Words,
    }
    fn reply(reader: Reader, mut request: Words) -> Reader {
        let mut digest: u64 = 0;
        for index in 0..shared_len(reader.bytes) {
            digest = digest * 31 + shared_read(reader.bytes, index);
        }
        // Move the same reader state through helpers, without cloning a share.
        write(request, 1, digest);
        send(input_target(), 201, request);
        return reader;
    }
    fn session() {
        let room_monitor = monitor(input_target());
        let mut reader = Reader {
            bytes: shared_bytes(receive_shared(7)),
            startup: owned_input(),
        };
        while true {
            match receive_event() {
                Event::Message(message) => {
                    if message_tag(message) == 100 {
                        reader = reply(reader, message_words(message));
                    } else if message_tag(message) == 900 {
                        send(input_target(), 202, message_words(message));
                        result(len(reader.startup));
                        return;
                    } else {
                        result(999);
                        return;
                    }
                }
                Event::Down(down) => {
                    if down_ref(down) == room_monitor && down_target(down) == input_target() {
                        result(shared_len(reader.bytes));
                        return;
                    }
                    result(999);
                }
                Event::Shared(unexpected_share) => {
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
    fn run() {
        // This host-known member is genuinely observed and receives a room
        // broadcast; it is not an unrelated background computation.
        let busy = input_target();
        let busy_monitor = monitor(busy);
        let peer = spawn_monitor(session, owned_input(), 0, 0);
        let resident = shared_bytes(receive_shared(7));
        send_shared(busy, 7, clone_shared(resident));
        send_shared(spawned_pid(peer), 7, clone_shared(resident));
        let mut busy_done = false;
        let mut finishing = false;
        let mut finish_ack = false;
        let mut sequence: u64 = 0;
        mark(10);
        while true {
            match receive_event() {
                Event::Message(message) => {
                    let tag = message_tag(message);
                    let words = message_words(message);
                    if tag == 100 && !finishing {
                        send(spawned_pid(peer), 100, words);
                    } else if tag == 201 && !finishing {
                        if read(words, 0) != sequence + 1 || read(words, 1) != 62627 {
                            result(999);
                            return;
                        }
                        sequence += 1;
                        result(read(words, 1) * 10 + sequence);
                        observe(sequence);
                    } else if tag == 900 && busy_done && !finishing {
                        finishing = true;
                        send(spawned_pid(peer), 900, words);
                    } else if tag == 202 && finishing && !finish_ack {
                        finish_ack = true;
                    } else {
                        result(999);
                        return;
                    }
                }
                Event::Down(down) => {
                    if down_ref(down) == busy_monitor
                        && down_target(down) == busy
                        && !busy_done
                        && down_reason(down) == 2
                    {
                        busy_done = true;
                        mark(20);
                    } else if down_ref(down) == spawned_ref(peer)
                        && down_target(down) == spawned_pid(peer)
                        && down_reason(down) == 1
                        && finishing
                        && finish_ack
                    {
                        result(626270 + sequence + shared_len(resident) - 4);
                        mark(30);
                        return;
                    } else {
                        result(999);
                        return;
                    }
                }
                Event::Shared(unexpected_share) => {
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
}

struct Fixture {
    runtime: Runtime,
    client: RuntimeClient,
    busy: ProcessHandle,
    room: ProcessHandle,
    host: SharedBytes,
    receipt: SharedReceipt,
    requests: Vec<MessageReceipt>,
    queued_words: Vec<MessageReceipt>,
    checkpoints: Vec<[u64; 4]>,
}
impl Fixture {
    fn new(allowance: u32) -> Self {
        let runtime = Runtime::paused(allowance);
        let client = runtime.client();
        let busy = client.spawn(computation::program(), vec![0; 4]).unwrap();
        let room = client
            .spawn_to(
                service::program(),
                Input {
                    words: vec![1],
                    tag: 0,
                    argument: 0,
                },
                busy.id(),
            )
            .unwrap();
        let (host, receipt) = SharedBytes::new(vec![2, 3, 5, 7]).unwrap();
        client.send_shared(room.id(), SharedEnvelope::new(7, host.clone()));
        runtime.start();
        wait(&busy, |s| s.stage == 70 && s.progress >= 2);
        wait(&room, |s| s.stage == 10 && s.waiting);
        assert_eq!(client.live_count(), 3);
        assert_eq!(client.active_monitors(), 3);
        assert_eq!(receipt.snapshot().references_created, 4);
        Self {
            runtime,
            client,
            busy,
            room,
            host,
            receipt,
            requests: Vec::new(),
            queued_words: Vec::new(),
            checkpoints: Vec::new(),
        }
    }
    fn request(&mut self, sequence: u64, stopped: bool) {
        let before = self.busy.snapshot();
        let (message, receipt) = packet(100, vec![sequence, 0]);
        self.requests.push(receipt);
        self.client.send(self.room.id(), message);
        let room = wait(&self.room, |s| s.progress == sequence && s.waiting);
        assert_eq!(room.result, 626270 + sequence);
        assert_eq!(room.stage, if stopped { 20 } else { 10 });
        if stopped {
            frozen_application(&before, &self.busy.snapshot());
        } else {
            // Progress amounts are never normalized by elapsed time or compared
            // between configurations. The finite protocol must finish while
            // the busy member stays live and continues its own logical work.
            let after = wait(&self.busy, |s| s.progress > before.progress);
            assert_eq!(
                (after.terminal, after.receives, after.waiting),
                (0, 1, false)
            );
            self.checkpoints
                .push([room.progress, room.result, before.progress, after.progress]);
        }
        assert_eq!(self.requests.last().unwrap().drops(), 1);
    }
    fn queue_work(&mut self) {
        for tag in 0..19 {
            self.client.send_shared(
                self.busy.id(),
                SharedEnvelope::new(1000 + tag, self.host.clone()),
            );
        }
        for value in 0..3 {
            let (message, receipt) = packet(2000, vec![value]);
            self.client.send(self.busy.id(), message);
            self.queued_words.push(receipt);
        }
        let s = self.busy.snapshot();
        assert_eq!((s.queued, s.receives, s.scanned_prefix), (22, 1, 0));
        let refs = self.receipt.snapshot();
        assert_eq!(
            (
                refs.live_references,
                refs.references_created,
                refs.references_dropped,
                refs.live_bytes,
                refs.physical_drops
            ),
            (23, 23, 0, 4, 0)
        );
    }
}

fn assert_input(s: &Snapshot, bytes: u64) {
    assert_eq!((s.drops, s.live_bytes, s.freed_bytes), (1, 0, bytes));
}
fn ref_row(s: SharedSnapshot) -> [u64; 6] {
    [
        s.live_bytes,
        s.freed_bytes,
        s.physical_drops,
        s.live_references,
        s.references_created,
        s.references_dropped,
    ]
}
fn assert_host_last(host: &SharedBytes, receipt: &SharedReceipt) -> [u64; 6] {
    let s = receipt.snapshot();
    assert_eq!(ref_row(s), [4, 0, 0, 1, 23, 22]);
    assert_eq!(
        (host.read(0), host.read(1), host.read(2), host.read(3)),
        (2, 3, 5, 7)
    );
    ref_row(s)
}
fn final_release(host: SharedBytes, receipt: &SharedReceipt) -> [u64; 6] {
    drop(host);
    let s = receipt.snapshot();
    assert_eq!(ref_row(s), [0, 4, 1, 0, 23, 23]);
    ref_row(s)
}

pub(super) fn run(allowance: u32) {
    let mut progress_observations = Vec::new();
    let mut post_stop_observations = Vec::new();
    let mut ownership_observations = Vec::new();
    let mut stopped_observations = Vec::new();
    for linked in [false, true] {
        let mut f = Fixture::new(allowance);
        for sequence in 1..=3 {
            f.request(sequence, false);
        }
        f.queue_work();
        let mut trigger = None;
        if linked {
            let partner = f.client.spawn(waiter::program(), vec![]).unwrap();
            wait(&partner, |s| s.waiting);
            f.client.link(f.busy.id(), partner.id()).unwrap();
            assert_eq!(partner.kill(), KillReceipt::Accepted);
            done(&partner, 2);
            trigger = Some(partner);
        } else {
            assert_eq!(f.busy.kill(), KillReceipt::Accepted);
        }
        let stopped = done(&f.busy, if linked { 3 } else { 2 });
        assert_eq!(stopped.result, 62627);
        assert_eq!(
            (
                stopped.exit_reason,
                stopped.receives,
                stopped.cleanup_messages,
                stopped.cleanup_batches
            ),
            (2, 1, 22, 3)
        );
        assert_eq!(stopped.kill_event == 0, linked);
        assert_input(&stopped, 32);
        let observed_down = wait(&f.room, |s| s.stage == 20 && s.waiting);
        assert_eq!(observed_down.progress, 3);
        assert!(!f.client.is_live(f.busy.id()));
        assert_eq!(f.client.active_monitors(), 2);
        assert_eq!(f.client.active_links(), 0);
        assert_eq!(f.client.live_count(), 2);
        let after_stop = f.receipt.snapshot();
        assert_eq!(ref_row(after_stop), [4, 0, 0, 3, 23, 20]);
        for sequence in 4..=5 {
            f.request(sequence, true);
            post_stop_observations.push([u64::from(linked), sequence, f.room.snapshot().result]);
        }
        frozen_application(&stopped, &f.busy.snapshot());
        assert_eq!(f.receipt.snapshot().references_created, 23); // Helpers only move the handle.
        let (finish, receipt) = packet(900, vec![0, 0]);
        f.requests.push(receipt);
        f.client.send(f.room.id(), finish);
        let room_done = done(&f.room, 1);
        assert_eq!(
            (room_done.stage, room_done.progress, room_done.result),
            (30, 5, 626275)
        );
        assert_input(&room_done, 8); // Startup allocation moved to the healthy child.
        let Fixture {
            runtime,
            client,
            busy,
            host,
            receipt,
            requests,
            queued_words,
            checkpoints,
            ..
        } = f;
        runtime.shutdown();
        frozen_application(&stopped, &busy.snapshot());
        assert_eq!(
            (
                client.live_count(),
                client.active_monitors(),
                client.active_links(),
                client.clock_snapshot().pending
            ),
            (0, 0, 0, 0)
        );
        released(&requests, 16);
        released(&queued_words, 8);
        if let Some(partner) = trigger {
            assert_input(&partner.snapshot(), 0);
        }
        progress_observations.push(checkpoints);
        ownership_observations.push([
            ref_row(after_stop),
            assert_host_last(&host, &receipt),
            final_release(host, &receipt),
        ]);
        stopped_observations.push([
            u64::from(linked),
            stopped.terminal as u64,
            stopped.exit_reason,
            stopped.receives,
            stopped.cleanup_messages,
            stopped.cleanup_batches,
            stopped.result,
        ]);
    }

    // Active shutdown: computation continues, reader is parked, room is live,
    // and both shared and ordinary work remain queued at the busy member.
    let mut f = Fixture::new(allowance);
    for sequence in 1..=3 {
        f.request(sequence, false);
    }
    f.queue_work();
    let before_shutdown = f.busy.snapshot();
    assert_eq!(
        (before_shutdown.terminal, before_shutdown.waiting),
        (0, false)
    );
    let Fixture {
        runtime,
        client,
        busy,
        room,
        host,
        receipt,
        requests,
        queued_words,
        checkpoints,
    } = f;
    runtime.shutdown();
    let busy_done = done(&busy, 2);
    assert_eq!(busy_done.result, 62627);
    let room_done = done(&room, 2);
    assert_eq!(
        (
            busy_done.receives,
            busy_done.cleanup_messages,
            busy_done.cleanup_batches
        ),
        (1, 22, 3)
    );
    assert_eq!((room_done.progress, room_done.result), (3, 626273));
    assert_input(&busy_done, 32);
    assert_input(&room_done, 8);
    assert_eq!(
        (
            client.live_count(),
            client.active_monitors(),
            client.active_links(),
            client.clock_snapshot().pending
        ),
        (0, 0, 0, 0)
    );
    released(&requests, 16);
    released(&queued_words, 8);
    let retained = assert_host_last(&host, &receipt);
    let freed = final_release(host, &receipt);
    frozen_application(&busy_done, &busy.snapshot());
    progress_observations.push(checkpoints);
    println!("{{\"kind\":\"busy_controls\",\"passed\":true,\"allowance\":{allowance},\"progress_observations\":{progress_observations:?},\"post_stop_observations\":{post_stop_observations:?},\"stop_observations\":{stopped_observations:?},\"ownership_observations\":{ownership_observations:?},\"shutdown_ownership\":[{retained:?},{freed:?}],\"shutdown_cleanup\":[{},{}],\"shutdown_busy_result\":{},\"shutdown_room_result\":{},\"exact_down_without_busy_receive\":true,\"no_shared_clones_during_computation_or_replies\":true,\"application_frozen_after_stop\":true,\"speed_tests\":false}}", busy_done.cleanup_messages, busy_done.cleanup_batches, busy_done.result, room_done.result);
}
