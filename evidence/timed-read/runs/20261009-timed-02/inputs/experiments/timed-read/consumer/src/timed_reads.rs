use super::*;
use bream_timed_runtime::{StreamReceipt, StreamSnapshot};
use std::io::Write;
use std::os::unix::net::UnixStream;

#[process_module]
mod once {
    fn hold_bytes(bytes: StreamBytes, words: Words) {
        let mut sum: u64 = 0;
        for i in 0..stream_len(bytes) {
            sum = sum * 31 + stream_byte(bytes, i);
        }
        result(sum);
        mark(3);
        let _gate_1 = receive(100);
        let mut again: u64 = 0;
        for j in 0..stream_len(bytes) {
            again = again * 31 + stream_byte(bytes, j);
        }
        result(again + len(words));
    }
    fn hold_code(code: u64, words: Words) {
        result(code);
        mark(3);
        let _gate_2 = receive(100);
        result(code + len(words));
    }
    fn run() {
        let words = owned_input();
        mark(1);
        let _gate_3 = receive(99);
        mark(2);
        if input_tag() == 1 {
            match read_next() {
                StreamResult::Data(bytes) => {
                    hold_bytes(bytes, words);
                }
                StreamResult::Eof => {
                    hold_code(701, words);
                }
                StreamResult::Failed => {
                    hold_code(702, words);
                }
                StreamResult::Unavailable => {
                    hold_code(703, words);
                }
                StreamResult::Busy => {
                    hold_code(704, words);
                }
            }
        } else {
            match read_next_for(input_argument()) {
                TimedStreamResult::Data(timed_bytes) => {
                    hold_bytes(timed_bytes, words);
                }
                TimedStreamResult::Eof => {
                    hold_code(701, words);
                }
                TimedStreamResult::Failed => {
                    hold_code(702, words);
                }
                TimedStreamResult::Unavailable => {
                    hold_code(703, words);
                }
                TimedStreamResult::Busy => {
                    hold_code(704, words);
                }
                TimedStreamResult::Timeout => {
                    hold_code(700, words);
                }
            }
        }
        mark(4);
    }
}
#[process_module]
mod sequence {
    fn finish_pair(first: StreamBytes, later: StreamBytes, words: Words) {
        let mut a: u64 = 0;
        let mut b: u64 = 0;
        for i in 0..stream_len(first) {
            a = a * 31 + stream_byte(first, i);
        }
        for j in 0..stream_len(later) {
            b = b * 31 + stream_byte(later, j);
        }
        result(a * 1000000 + b);
        mark(7);
        let _gate_4 = receive(102);
        mark(8);
        match read_next_for(0) {
            TimedStreamResult::Eof => {
                observe(2);
            }
            TimedStreamResult::Data(unexpected_1) => {
                result(990);
                return;
            }
            TimedStreamResult::Failed => {
                result(991);
                return;
            }
            TimedStreamResult::Unavailable => {
                result(992);
                return;
            }
            TimedStreamResult::Busy => {
                result(993);
                return;
            }
            TimedStreamResult::Timeout => {
                result(994);
                return;
            }
        }
        match read_next_for(5) {
            TimedStreamResult::Eof => {
                observe(3);
            }
            TimedStreamResult::Data(unexpected_2) => {
                result(990);
                return;
            }
            TimedStreamResult::Failed => {
                result(991);
                return;
            }
            TimedStreamResult::Unavailable => {
                result(992);
                return;
            }
            TimedStreamResult::Busy => {
                result(993);
                return;
            }
            TimedStreamResult::Timeout => {
                result(994);
                return;
            }
        }
        result(stream_byte(first, 0) + stream_byte(later, 2) + len(words));
        mark(9);
    }
    fn keep_first(first: StreamBytes, words: Words) {
        let mut sum: u64 = 0;
        for i in 0..stream_len(first) {
            sum = sum * 31 + stream_byte(first, i);
        }
        result(sum);
        mark(3);
        let _gate_5 = receive(100);
        mark(4);
        match read_next_for(5) {
            TimedStreamResult::Timeout => {
                observe(1);
            }
            TimedStreamResult::Data(unexpected_3) => {
                result(990);
                return;
            }
            TimedStreamResult::Eof => {
                result(995);
                return;
            }
            TimedStreamResult::Failed => {
                result(991);
                return;
            }
            TimedStreamResult::Unavailable => {
                result(992);
                return;
            }
            TimedStreamResult::Busy => {
                result(993);
                return;
            }
        }
        mark(5);
        let _gate_6 = receive(101);
        mark(6);
        match read_next_for(20) {
            TimedStreamResult::Data(later) => {
                finish_pair(first, later, words);
            }
            TimedStreamResult::Timeout => {
                result(994);
            }
            TimedStreamResult::Eof => {
                result(995);
            }
            TimedStreamResult::Failed => {
                result(991);
            }
            TimedStreamResult::Unavailable => {
                result(992);
            }
            TimedStreamResult::Busy => {
                result(993);
            }
        }
    }
    fn run() {
        let words = owned_input();
        mark(1);
        let _gate_7 = receive(99);
        mark(2);
        match read_next_for(5) {
            TimedStreamResult::Data(first) => {
                keep_first(first, words);
            }
            TimedStreamResult::Timeout => {
                result(994);
            }
            TimedStreamResult::Eof => {
                result(995);
            }
            TimedStreamResult::Failed => {
                result(991);
            }
            TimedStreamResult::Unavailable => {
                result(992);
            }
            TimedStreamResult::Busy => {
                result(993);
            }
        }
    }
}
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !condition() {
        assert!(Instant::now() < deadline, "timed-read correctness watchdog");
        std::thread::yield_now();
    }
}
fn gate(rt: &Runtime, h: &ProcessHandle, tag: u64) {
    rt.client().send(h.id(), packet(tag, vec![]).0);
}
fn stage(h: &ProcessHandle, n: u64) -> Snapshot {
    wait(h, |s| s.stage == n && s.waiting)
}
fn install(rt: &Runtime, h: &ProcessHandle) -> (UnixStream, StreamReceipt) {
    let (socket, peer) = UnixStream::pair().unwrap();
    let r = rt.client().install_stream(h.id(), socket, 4).unwrap();
    (peer, r)
}
fn actor(rt: &Runtime) -> (ProcessHandle, UnixStream, StreamReceipt) {
    let h = rt.spawn(sequence::program(), vec![11, 13]).unwrap();
    stage(&h, 1);
    let (peer, r) = install(rt, &h);
    (h, peer, r)
}
fn snapshot(r: &StreamReceipt) -> StreamSnapshot {
    until(|| {
        let s = r.snapshot();
        s.retired && s.live_bytes == 0 && s.drops == s.buffers
    });
    r.snapshot()
}
fn row(r: &StreamReceipt) -> Vec<u64> {
    let s = snapshot(r);
    assert_eq!(s.started, s.settled);
    assert_eq!(s.started, s.delivered + s.discarded);
    assert_eq!(
        (s.buffers, s.drops, s.endpoint_drops, s.active_id),
        (s.started, s.started, 1, 0)
    );
    vec![
        s.started,
        s.settled,
        s.delivered,
        s.discarded,
        s.timeouts,
        s.drops,
        s.live_bytes,
        s.endpoint_drops,
    ]
}
fn first(rt: &Runtime, h: &ProcessHandle, peer: &mut UnixStream) {
    peer.write_all(&[2, 3, 5, 7]).unwrap();
    gate(rt, h, 99);
    assert_eq!(stage(h, 3).result, 62627);
}
fn expire_second(rt: &Runtime, h: &ProcessHandle) {
    gate(rt, h, 100);
    stage(h, 4);
    until(|| rt.client().clock_snapshot().pending == 1);
    rt.client().advance_to(4).unwrap();
    assert_eq!(rt.client().clock_snapshot().expired, 0);
    rt.client().advance_to(5).unwrap();
    assert_eq!(stage(h, 5).progress, 1);
}
pub fn run(allowance: u32) {
    let mut simple = Vec::new();
    for mode in 0..7 {
        let rt = Runtime::with_stream_capacity(allowance, 3, 1);
        let ticks = if mode == 2 || mode == 3 { 5 } else { 0 };
        let h = rt
            .spawn_input(
                once::program(),
                Input {
                    words: vec![11, 13],
                    tag: if mode == 4 { 1 } else { 0 },
                    argument: ticks,
                },
            )
            .unwrap();
        stage(&h, 1);
        let (mut peer, r) = install(&rt, &h);
        match mode {
            1 => peer.write_all(&[2, 3, 5]).unwrap(),
            4 => peer.write_all(&[17, 19, 23]).unwrap(),
            5 => peer.write_all(&[41, 43]).unwrap(),
            _ => {}
        }
        let mut peer = Some(peer);
        if mode == 6 {
            drop(peer.take());
        }
        gate(&rt, &h, 99);
        if mode == 2 || mode == 3 {
            stage(&h, 2);
            until(|| r.snapshot().attempts > 0 && rt.client().clock_snapshot().pending == 1);
            let p = rt.spawn(peer::program(), vec![]).unwrap();
            assert_eq!(done(&p, 1).result, 9);
            if mode == 2 {
                rt.client().advance_to(5).unwrap();
            } else {
                peer.as_mut().unwrap().write_all(&[7, 11, 13]).unwrap();
            }
        }
        let held = stage(&h, 3);
        let expected = [700, 2020, 700, 7081, 16949, 1314, 701][mode];
        assert_eq!(held.result, expected);
        let clock = rt.client().clock_snapshot();
        assert_eq!(clock.pending, 0);
        gate(&rt, &h, 100);
        assert_eq!(done(&h, 1).result, expected + 2);
        let s = snapshot(&r);
        assert_eq!((s.started, s.delivered, s.drops), (1, 1, 1));
        simple.push(vec![
            mode as u64,
            expected,
            s.started,
            s.delivered,
            s.timeouts,
            clock.armed,
            clock.expired,
            clock.cancelled,
        ]);
        rt.shutdown();
    }
    let rt = Runtime::with_stream_capacity(allowance, 3, 1);
    let (h, mut peer, r) = actor(&rt);
    first(&rt, &h, &mut peer);
    expire_second(&rt, &h);
    assert_eq!(
        (r.snapshot().live_bytes, rt.client().active_streams()),
        (4, 1)
    );
    gate(&rt, &h, 101);
    stage(&h, 6);
    until(|| rt.client().clock_snapshot().pending == 1);
    let p = rt.spawn(peer::program(), vec![]).unwrap();
    assert_eq!(done(&p, 1).result, 9);
    peer.write_all(&[11, 13, 17]).unwrap();
    assert_eq!(stage(&h, 7).result, 62627010991);
    assert_eq!(r.snapshot().live_bytes, 8);
    drop(peer);
    gate(&rt, &h, 102);
    let done = done(&h, 1);
    assert_eq!(
        (done.stage, done.progress, done.result, done.drops),
        (9, 3, 21, 1)
    );
    let sequence = row(&r);
    assert_eq!(r.snapshot().last_id, 4);
    let c = rt.client().clock_snapshot();
    let sequence_clock = vec![c.armed, c.expired, c.cancelled, c.pending as u64];
    rt.shutdown();
    let mut stops = Vec::new();
    for mode in 0..7 {
        let rt = Runtime::with_stream_capacity(allowance, 3, 1);
        let (h, mut peer, r) = actor(&rt);
        if mode >= 2 {
            first(&rt, &h, &mut peer);
        } else if mode == 1 {
            gate(&rt, &h, 99);
            stage(&h, 2);
        }
        if mode == 3 {
            gate(&rt, &h, 100);
            stage(&h, 4);
        }
        if mode >= 4 {
            expire_second(&rt, &h);
        }
        if mode >= 5 {
            gate(&rt, &h, 101);
            stage(&h, 6);
        }
        if mode == 6 {
            peer.write_all(&[11, 13, 17]).unwrap();
            stage(&h, 7);
        }
        if [1, 3, 5].contains(&mode) {
            until(|| rt.client().clock_snapshot().pending == 1);
        }
        let mut receipts = Vec::new();
        for _ in 0..19 {
            let (m, r) = packet(88, vec![5]);
            rt.client().send(h.id(), m);
            receipts.push(r);
        }
        h.kill();
        let stopped = super::done(&h, 2);
        assert_eq!(
            (
                stopped.cleanup_messages,
                stopped.cleanup_batches,
                stopped.drops
            ),
            (19, 3, 1)
        );
        released(&receipts, 8);
        let s = snapshot(&r);
        assert_eq!(rt.client().clock_snapshot().pending, 0);
        let p = rt.spawn(peer::program(), vec![]).unwrap();
        assert_eq!(super::done(&p, 1).result, 9);
        frozen_application(&stopped, &h.snapshot());
        stops.push(vec![
            mode,
            stopped.stage,
            s.started,
            s.delivered,
            s.discarded,
            s.timeouts,
            s.drops,
            s.endpoint_drops,
            stopped.cleanup_messages,
            stopped.cleanup_batches,
        ]);
        rt.shutdown();
    }
    // Build the longest pending request before installing the other two states.
    let rt = Runtime::with_stream_capacity(allowance, 4, 3);
    let client = rt.client();
    let (a, mut ap, ar) = actor(&rt);
    first(&rt, &a, &mut ap);
    expire_second(&rt, &a);
    gate(&rt, &a, 101);
    stage(&a, 6);
    let (b, bp, br) = actor(&rt);
    gate(&rt, &b, 99);
    stage(&b, 2);
    let (c, cp, cr) = actor(&rt);
    until(|| client.clock_snapshot().pending == 2);
    rt.shutdown();
    for h in [&a, &b, &c] {
        super::done(h, 2);
    }
    let shutdown = vec![row(&ar), row(&br), row(&cr)];
    assert_eq!(client.clock_snapshot().pending, 0);
    assert!(client.stream_driver_stopped());
    drop((ap, bp, cp));
    let mut race_data = 0;
    let mut kill_deliveries = 0;
    for _ in 0..16 {
        let rt = Runtime::with_stream_capacity(allowance, 2, 1);
        let client = rt.client();
        let h = rt
            .spawn_input(
                once::program(),
                Input {
                    words: vec![11, 13],
                    tag: 0,
                    argument: 5,
                },
            )
            .unwrap();
        stage(&h, 1);
        let (mut peer, r) = install(&rt, &h);
        gate(&rt, &h, 99);
        stage(&h, 2);
        until(|| client.clock_snapshot().pending == 1);
        let barrier = Arc::new(Barrier::new(2));
        let ready = barrier.clone();
        let writer = std::thread::spawn(move || {
            ready.wait();
            peer.write_all(&[41]).unwrap();
            peer
        });
        barrier.wait();
        client.advance_to(5).unwrap();
        let _peer = writer.join().unwrap();
        let s = stage(&h, 3);
        assert!([700, 41].contains(&s.result));
        race_data += u64::from(s.result == 41);
        gate(&rt, &h, 100);
        super::done(&h, 1);
        let s = snapshot(&r);
        assert_eq!(
            (s.started, s.settled, s.delivered, s.discarded, s.drops),
            (1, 1, 1, 0, 1)
        );
        assert_eq!(s.timeouts, u64::from(h.snapshot().result == 702));
        assert_eq!(client.clock_snapshot().pending, 0);
        rt.shutdown();
    }
    for _ in 0..16 {
        let rt = Runtime::with_stream_capacity(allowance, 2, 1);
        let client = rt.client();
        let h = rt
            .spawn_input(
                once::program(),
                Input {
                    words: vec![11, 13],
                    tag: 0,
                    argument: 5,
                },
            )
            .unwrap();
        stage(&h, 1);
        let (_peer, r) = install(&rt, &h);
        gate(&rt, &h, 99);
        stage(&h, 2);
        until(|| client.clock_snapshot().pending == 1);
        let barrier = Arc::new(Barrier::new(2));
        let ready = barrier.clone();
        let clock = client.clone();
        let expiry = std::thread::spawn(move || {
            ready.wait();
            clock.advance_to(5).unwrap();
        });
        barrier.wait();
        h.kill();
        expiry.join().unwrap();
        super::done(&h, 2);
        let s = snapshot(&r);
        assert_eq!(
            (s.started, s.settled, s.delivered + s.discarded, s.drops),
            (1, 1, 1, 1)
        );
        kill_deliveries += s.delivered;
        assert_eq!(client.clock_snapshot().pending, 0);
        rt.shutdown();
    }
    println!("{{\"kind\":\"timed_read_controls\",\"passed\":true,\"allowance\":{allowance},\"simple\":{simple:?},\"sequence\":{sequence:?},\"sequence_clock\":{sequence_clock:?},\"stops\":{stops:?},\"shutdown\":{shutdown:?},\"peer_progress\":true,\"data_timeout_races\":16,\"race_data_deliveries\":{race_data},\"kill_timeout_races\":16,\"race_kill_deliveries\":{kill_deliveries},\"speed_tests\":false}}");
}
