use super::*;
use bream_stream_runtime::{ReadAdmissionError, StreamReceipt, StreamSnapshot};
use std::io::Write;
use std::os::unix::net::UnixStream;
#[process_module]
mod sequence {
    fn finish(first: StreamBytes, second: StreamBytes, words: Words) {
        let mut a: u64 = 0;
        for i in 0..stream_len(first) {
            a = a * 31 + stream_byte(first, i);
        }
        let mut b: u64 = 0;
        for j in 0..stream_len(second) {
            b = b * 31 + stream_byte(second, j);
        }
        result(a * 1000000 + b);
        mark(5);
        let _gate = receive(101);
        mark(6);
        match read_next() {
            StreamResult::Eof => {
                observe(1);
            }
            StreamResult::Data(unexpected) => {
                result(999);
                return;
            }
            StreamResult::Failed => {
                result(998);
                return;
            }
            StreamResult::Unavailable => {
                result(997);
                return;
            }
            StreamResult::Busy => {
                result(996);
                return;
            }
        }
        match read_next() {
            StreamResult::Eof => {
                observe(2);
            }
            StreamResult::Data(unexpected_again) => {
                result(999);
                return;
            }
            StreamResult::Failed => {
                result(998);
                return;
            }
            StreamResult::Unavailable => {
                result(997);
                return;
            }
            StreamResult::Busy => {
                result(996);
                return;
            }
        }
        if stream_byte(first, 0) + stream_byte(second, 0) + len(words) != 15 {
            result(995);
            return;
        }
        mark(7);
    }
    fn await_second(first: StreamBytes, words: Words) {
        let mut checksum: u64 = 0;
        for i in 0..stream_len(first) {
            checksum = checksum * 31 + stream_byte(first, i);
        }
        result(checksum);
        mark(3);
        let _gate = receive(100);
        mark(4);
        match read_next() {
            StreamResult::Data(bytes) => {
                finish(first, bytes, words);
            }
            StreamResult::Eof => {
                result(900);
            }
            StreamResult::Failed => {
                result(901);
            }
            StreamResult::Unavailable => {
                result(902);
            }
            StreamResult::Busy => {
                result(903);
            }
        }
    }
    fn run() {
        let words = owned_input();
        mark(1);
        let _gate = receive(99);
        mark(2);
        match read_next() {
            StreamResult::Data(bytes) => {
                await_second(bytes, words);
            }
            StreamResult::Eof => {
                result(90);
            }
            StreamResult::Failed => {
                result(91);
            }
            StreamResult::Unavailable => {
                result(92);
            }
            StreamResult::Busy => {
                result(93);
            }
        }
    }
}
fn wait_stream(r: &StreamReceipt, p: impl Fn(StreamSnapshot) -> bool) -> StreamSnapshot {
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        let s = r.snapshot();
        if p(s) {
            return s;
        }
        assert!(Instant::now() < end, "stream correctness watchdog");
        std::thread::yield_now();
    }
}
fn gate(rt: &Runtime, h: &ProcessHandle, tag: u64) {
    rt.client().send(h.id(), packet(tag, vec![]).0);
}
fn stage(h: &ProcessHandle, n: u64) -> Snapshot {
    wait(h, |s| s.stage == n && s.waiting)
}
fn actor(rt: &Runtime) -> ProcessHandle {
    let h = rt.spawn(sequence::program(), vec![11, 13]).unwrap();
    stage(&h, 1);
    h
}
fn install(rt: &Runtime, h: &ProcessHandle) -> (UnixStream, StreamReceipt) {
    let (reader, writer) = UnixStream::pair().unwrap();
    let r = rt.client().install_stream(h.id(), reader, 4).unwrap();
    (writer, r)
}
fn balance(r: &StreamReceipt, started: u64, delivered: u64) -> Vec<u64> {
    let s = wait_stream(r, |s| {
        s.retired && s.live_bytes == 0 && s.drops == s.buffers
    });
    assert_eq!(
        (s.started, s.settled, s.delivered, s.discarded),
        (started, started, delivered, started - delivered)
    );
    assert_eq!(
        (s.buffers, s.drops, s.endpoint_drops, s.active_id),
        (started, started, 1, 0)
    );
    vec![
        s.started,
        s.settled,
        s.delivered,
        s.discarded,
        s.buffers,
        s.drops,
        s.live_bytes,
        s.endpoint_drops,
        s.retired as u64,
    ]
}
fn end(rt: Runtime, h: ProcessHandle, writer: UnixStream, r: StreamReceipt) -> Vec<u64> {
    let held = stage(&h, 5);
    assert_eq!(held.result, 62627010991);
    assert_eq!(
        (r.snapshot().live_bytes, r.snapshot().endpoint_drops),
        (8, 0)
    );
    drop(writer);
    gate(&rt, &h, 101);
    let stopped = done(&h, 1);
    assert_eq!(
        (
            stopped.stage,
            stopped.progress,
            stopped.result,
            stopped.drops
        ),
        (7, 2, 62627010991, 1)
    );
    let row = balance(&r, 3, 3);
    assert_eq!(r.snapshot().last_id, 3);
    rt.shutdown();
    row
}
fn complete(rt: Runtime, h: ProcessHandle, mut writer: UnixStream, r: StreamReceipt) -> Vec<u64> {
    writer.write_all(&[2, 3, 5, 7, 11, 13, 17]).unwrap();
    gate(&rt, &h, 99);
    stage(&h, 3);
    gate(&rt, &h, 100);
    end(rt, h, writer, r)
}
pub fn run(allowance: u32) {
    let mut completed = Vec::new();
    let mut request_ids = Vec::new();
    // A four-byte read leaves three buffered bytes for a later request.
    let rt = Runtime::with_stream_capacity(allowance, 4, 2);
    let h = actor(&rt);
    let (mut writer, r) = install(&rt, &h);
    assert_eq!(
        (
            r.snapshot().started,
            r.snapshot().buffers,
            rt.client().active_streams()
        ),
        (0, 0, 1)
    );
    writer.write_all(&[2, 3, 5, 7, 11, 13, 17]).unwrap();
    gate(&rt, &h, 99);
    stage(&h, 3);
    request_ids.push(r.snapshot().last_id);
    assert_eq!(r.snapshot().live_bytes, 4);
    gate(&rt, &h, 100);
    stage(&h, 5);
    request_ids.push(r.snapshot().last_id);
    completed.push(end(rt, h, writer, r));
    // Distinct writes: while the second request waits, the first buffer survives.
    let rt = Runtime::with_stream_capacity(allowance, 4, 2);
    let h = actor(&rt);
    let (mut writer, r) = install(&rt, &h);
    gate(&rt, &h, 99);
    stage(&h, 2);
    let peer = rt.spawn(peer::program(), vec![]).unwrap();
    assert_eq!(done(&peer, 1).result, 9);
    writer.write_all(&[2, 3, 5, 7]).unwrap();
    stage(&h, 3);
    gate(&rt, &h, 100);
    stage(&h, 4);
    assert_eq!(
        (
            r.snapshot().started,
            r.snapshot().delivered,
            r.snapshot().live_bytes
        ),
        (2, 1, 8)
    );
    let peer = rt.spawn(peer::program(), vec![]).unwrap();
    assert_eq!(done(&peer, 1).result, 9);
    writer.write_all(&[11, 13, 17]).unwrap();
    completed.push(end(rt, h, writer, r));
    let mut stops = Vec::new();
    for mode in 0..6 {
        let rt = Runtime::with_stream_capacity(allowance, 3, 1);
        let h = actor(&rt);
        let (mut writer, r) = install(&rt, &h);
        if mode >= 2 {
            writer.write_all(&[2, 3, 5, 7]).unwrap();
        }
        if mode >= 1 {
            gate(&rt, &h, 99);
            stage(&h, if mode >= 2 { 3 } else { 2 });
        }
        if mode >= 4 {
            writer.write_all(&[11, 13, 17]).unwrap();
        }
        if mode >= 3 {
            gate(&rt, &h, 100);
            stage(&h, if mode >= 4 { 5 } else { 4 });
        }
        if mode == 5 {
            gate(&rt, &h, 101);
            stage(&h, 6);
        }
        let mut queued = Vec::new();
        for _ in 0..19 {
            let (m, r) = packet(88, vec![5]);
            rt.client().send(h.id(), m);
            queued.push(r);
        }
        wait(&h, |s| s.waiting && s.queued == 19);
        let expected_started = [0, 1, 1, 2, 2, 3][mode as usize];
        let expected_delivered = [0, 0, 1, 1, 2, 2][mode as usize];
        h.kill();
        let stopped = done(&h, 2);
        let ownership = balance(&r, expected_started, expected_delivered);
        released(&queued, 8);
        assert_eq!(
            (
                stopped.cleanup_messages,
                stopped.cleanup_batches,
                stopped.drops,
                stopped.live_bytes
            ),
            (19, 3, 1, 0)
        );
        let peer = rt.spawn(peer::program(), vec![]).unwrap();
        assert_eq!(done(&peer, 1).result, 9);
        frozen_application(&stopped, &h.snapshot());
        stops.push(vec![
            mode,
            stopped.stage,
            ownership[0],
            ownership[2],
            ownership[3],
            ownership[5],
            ownership[7],
            stopped.cleanup_messages,
            stopped.cleanup_batches,
        ]);
        rt.shutdown();
    }
    // Consuming a result frees the request, not the still-open endpoint slot.
    let rt = Runtime::with_stream_capacity(allowance, 3, 1);
    let a = actor(&rt);
    let b = actor(&rt);
    let (mut writer, ra) = install(&rt, &a);
    writer.write_all(&[2, 3, 5, 7]).unwrap();
    gate(&rt, &a, 99);
    stage(&a, 3);
    assert_eq!(ra.snapshot().active_id, 0);
    let (reader, _peer) = UnixStream::pair().unwrap();
    assert!(matches!(
        rt.client().install_stream(b.id(), reader, 4),
        Err(ReadAdmissionError::Capacity)
    ));
    a.kill();
    done(&a, 2);
    balance(&ra, 1, 1);
    let (writer, rb) = install(&rt, &b);
    assert_eq!(complete(rt, b, writer, rb), completed[0]);
    let rt = Runtime::with_stream_capacity(allowance, 1, 1);
    let old = actor(&rt);
    let (_old_writer, old_r) = install(&rt, &old);
    gate(&rt, &old, 99);
    stage(&old, 2);
    old.kill();
    done(&old, 2);
    balance(&old_r, 1, 0);
    let new = actor(&rt);
    assert_eq!(old.id().slot(), new.id().slot());
    assert_ne!(old.id(), new.id());
    let (reader, _peer) = UnixStream::pair().unwrap();
    assert!(matches!(
        rt.client().install_stream(old.id(), reader, 4),
        Err(ReadAdmissionError::NotLive)
    ));
    let (writer, r) = install(&rt, &new);
    assert_eq!(complete(rt, new, writer, r), completed[0]);
    let rt = Runtime::with_stream_capacity(allowance, 3, 3);
    let client = rt.client();
    let a = actor(&rt);
    let b = actor(&rt);
    let c = actor(&rt);
    let (_wa, ra) = install(&rt, &a);
    let (_wb, rb) = install(&rt, &b);
    let (mut wc, rc) = install(&rt, &c);
    gate(&rt, &b, 99);
    stage(&b, 2);
    wc.write_all(&[2, 3, 5, 7]).unwrap();
    gate(&rt, &c, 99);
    stage(&c, 3);
    gate(&rt, &c, 100);
    stage(&c, 4);
    rt.shutdown();
    let shutdown = vec![balance(&ra, 0, 0), balance(&rb, 1, 0), balance(&rc, 2, 1)];
    assert!(client.stream_driver_stopped());
    assert_eq!(client.active_streams(), 0);
    done(&a, 2);
    done(&b, 2);
    done(&c, 2);
    let mut race_deliveries = 0;
    for _ in 0..16 {
        let rt = Runtime::with_stream_capacity(allowance, 2, 1);
        let h = actor(&rt);
        let (mut writer, r) = install(&rt, &h);
        writer.write_all(&[2, 3, 5, 7]).unwrap();
        gate(&rt, &h, 99);
        stage(&h, 3);
        gate(&rt, &h, 100);
        stage(&h, 4);
        let barrier = Arc::new(Barrier::new(3));
        let writer_gate = barrier.clone();
        let kill_gate = barrier.clone();
        let kill = h.clone();
        let send = std::thread::spawn(move || {
            writer_gate.wait();
            match writer.write_all(&[11, 13, 17]) {
                Ok(()) => {}
                Err(e) => assert!(matches!(
                    e.kind(),
                    std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                )),
            }
        });
        let killer = std::thread::spawn(move || {
            kill_gate.wait();
            kill.kill();
        });
        barrier.wait();
        send.join().unwrap();
        killer.join().unwrap();
        let stopped = done(&h, 2);
        assert!(stopped.stage == 4 || stopped.stage == 5);
        let settled = wait_stream(&r, |s| {
            s.retired && s.live_bytes == 0 && s.drops == s.buffers
        });
        assert!((1..=2).contains(&settled.delivered));
        balance(&r, 2, settled.delivered);
        race_deliveries += settled.delivered - 1;
        frozen_application(&stopped, &h.snapshot());
        rt.shutdown();
    }
    println!("{{\"kind\":\"stream_controls\",\"passed\":true,\"speed_tests\":false,\"allowance\":{},\"completed\":{:?},\"request_ids\":{:?},\"stops\":{:?},\"shutdown\":{:?},\"capacity_reused\":true,\"stale_isolated\":true,\"peer_progress\":true,\"second_read_kill_races\":16,\"race_second_deliveries\":{}}}",allowance,completed,request_ids,stops,shutdown,race_deliveries);
}
