use super::*;
use bream_stream_runtime::{ReadAdmissionError, ReadReceipt, ReadSnapshot};
use std::io::Write;
use std::os::unix::net::UnixStream;

#[process_module]
mod reader {
    fn hold(bytes: ReadBytes, words: Words) {
        let mut checksum: u64 = 0;
        for i in 0..read_len(bytes) {
            checksum = checksum * 31 + read_byte(bytes, i);
        }
        result(checksum + len(words));
        mark(3);
        let _gate = receive(100);
        let mut again: u64 = 0;
        for j in 0..read_len(bytes) {
            again = again * 31 + read_byte(bytes, j);
        }
        result(again + len(words));
        mark(4);
    }
    fn run() {
        let words = owned_input();
        mark(1);
        let _gate = receive(99);
        mark(2);
        match read_input() {
            ReadResult::Data(bytes) => {
                hold(bytes, words);
            }
            ReadResult::Eof => {
                result(0);
                mark(4);
            }
            ReadResult::Failed => {
                result(80);
                mark(4);
            }
            ReadResult::Unavailable => {
                result(90);
                mark(4);
            }
        }
        match read_input() {
            ReadResult::Data(unexpected) => {
                result(999);
            }
            ReadResult::Eof => {
                result(999);
            }
            ReadResult::Failed => {
                result(999);
            }
            ReadResult::Unavailable => {
                observe(1);
            }
        }
        mark(5);
    }
}
fn wait_read(receipt: &ReadReceipt, predicate: impl Fn(ReadSnapshot) -> bool) -> ReadSnapshot {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let s = receipt.snapshot();
        if predicate(s) {
            return s;
        }
        assert!(Instant::now() < deadline, "read correctness watchdog");
        std::thread::yield_now();
    }
}
fn gate(rt: &Runtime, actor: &ProcessHandle, tag: u64) {
    rt.client().send(actor.id(), packet(tag, vec![]).0);
}
fn actor(rt: &Runtime) -> ProcessHandle {
    let handle = rt.spawn(reader::program(), vec![11, 13]).unwrap();
    wait(&handle, |s| s.stage == 1 && s.waiting);
    handle
}
fn install(rt: &Runtime, actor: &ProcessHandle) -> (UnixStream, ReadReceipt) {
    let (read, write) = UnixStream::pair().unwrap();
    let receipt = rt.client().install_read(actor.id(), read, 8).unwrap();
    (write, receipt)
}
fn balanced(receipt: &ReadReceipt, delivered: bool) -> ReadSnapshot {
    let s = wait_read(receipt, |s| s.retired && s.storage_drops == 1);
    assert_eq!(
        (
            s.settled,
            s.delivered,
            s.live_bytes,
            s.storage_drops,
            s.endpoint_drops
        ),
        (true, delivered, 0, 1, 1)
    );
    s
}
fn ordinary_end(rt: Runtime, actor: ProcessHandle, receipt: ReadReceipt) -> Vec<u64> {
    let at = wait(&actor, |s| s.stage == 3 && s.waiting);
    assert_eq!(at.result, 62629);
    wait_read(&receipt, |s| s.retired);
    assert_eq!(
        (
            receipt.snapshot().live_bytes,
            receipt.snapshot().storage_drops
        ),
        (8, 0)
    );
    gate(&rt, &actor, 100);
    let stopped = done(&actor, 1);
    assert_eq!(
        (stopped.stage, stopped.result, stopped.progress),
        (5, 62629, 1)
    );
    let r = balanced(&receipt, true);
    rt.shutdown();
    vec![
        stopped.terminal as u64,
        stopped.result,
        stopped.progress,
        r.live_bytes,
        r.storage_drops,
        r.endpoint_drops,
    ]
}
pub fn run(allowance: u32) {
    let mut completions = Vec::new();
    // Pending read permits an independent checked peer to complete.
    let rt = Runtime::with_read_capacity(allowance, 4, 2);
    let a = actor(&rt);
    let (mut writer, receipt) = install(&rt, &a);
    gate(&rt, &a, 99);
    wait(&a, |s| s.stage == 2 && s.waiting);
    wait_read(&receipt, |s| s.attempts > 0);
    assert!(!receipt.snapshot().settled);
    let peer = rt.spawn(peer::program(), vec![]).unwrap();
    assert_eq!(done(&peer, 1).result, 9);
    writer.write_all(&[2, 3, 5, 7]).unwrap();
    drop(writer); // buffered data survive HUP
    completions.push(ordinary_end(rt, a, receipt));
    // Completion before the actor enters read_input retains the permit.
    let rt = Runtime::with_read_capacity(allowance, 3, 1);
    let a = actor(&rt);
    let (mut writer, receipt) = install(&rt, &a);
    writer.write_all(&[2, 3, 5, 7]).unwrap();
    wait_read(&receipt, |s| s.settled);
    assert_eq!(rt.client().active_reads(), 1);
    assert!(!receipt.snapshot().delivered);
    gate(&rt, &a, 99);
    completions.push(ordinary_end(rt, a, receipt));
    // EOF and no configured input have distinct typed branches.
    let rt = Runtime::with_read_capacity(allowance, 2, 1);
    let a = actor(&rt);
    let (writer, receipt) = install(&rt, &a);
    drop(writer);
    gate(&rt, &a, 99);
    let end = done(&a, 1);
    assert_eq!((end.result, end.progress), (0, 1));
    balanced(&receipt, true);
    rt.shutdown();
    let rt = Runtime::new(allowance);
    let a = actor(&rt);
    gate(&rt, &a, 99);
    let end = done(&a, 1);
    assert_eq!((end.result, end.progress), (90, 1));
    rt.shutdown();

    let mut stops = Vec::new();
    // 0 waiting in read; 1 ready result before read; 2 bytes moved into helper.
    for mode in 0..3 {
        let rt = Runtime::with_read_capacity(allowance, 3, 1);
        let a = actor(&rt);
        let (mut writer, receipt) = install(&rt, &a);
        if mode != 0 {
            writer.write_all(&[2, 3, 5, 7]).unwrap();
            wait_read(&receipt, |s| s.settled);
        }
        if mode != 1 {
            gate(&rt, &a, 99);
            wait(&a, |s| {
                s.stage == if mode == 2 { 3 } else { 2 } && s.waiting
            });
        }
        if mode == 0 {
            wait_read(&receipt, |s| s.attempts > 0);
        }
        if mode == 2 {
            wait_read(&receipt, |s| s.retired);
            assert_eq!(receipt.snapshot().live_bytes, 8);
        }
        let mut queued = Vec::new();
        for _ in 0..19 {
            let (m, r) = packet(88, vec![5]);
            rt.client().send(a.id(), m);
            queued.push(r);
        }
        // The selected wait rescans unrelated mailbox traffic before parking.
        wait(&a, |s| s.waiting && s.queued == 19);
        a.kill();
        let end = done(&a, 2);
        let r = balanced(&receipt, mode == 2);
        released(&queued, 8);
        assert_eq!((end.cleanup_messages, end.cleanup_batches), (19, 3));
        assert_eq!((end.drops, end.live_bytes), (1, 0));
        let p = rt.spawn(peer::program(), vec![]).unwrap();
        assert_eq!(done(&p, 1).result, 9);
        frozen_application(&end, &a.snapshot());
        stops.push(vec![
            mode,
            end.terminal as u64,
            end.stage,
            r.delivered as u64,
            end.cleanup_messages,
            end.cleanup_batches,
            r.storage_drops,
            r.endpoint_drops,
        ]);
        rt.shutdown();
    }
    // Ready but unconsumed storage must continue to consume admission capacity.
    let rt = Runtime::with_read_capacity(allowance, 3, 1);
    let a = actor(&rt);
    let b = actor(&rt);
    let (mut writer, first) = install(&rt, &a);
    writer.write_all(&[2, 3, 5, 7]).unwrap();
    wait_read(&first, |s| s.settled);
    let (read, _peer) = UnixStream::pair().unwrap();
    assert!(matches!(
        rt.client().install_read(b.id(), read, 8),
        Err(ReadAdmissionError::Capacity)
    ));
    a.kill();
    done(&a, 2);
    balanced(&first, false);
    let (mut writer, second) = install(&rt, &b);
    writer.write_all(&[2, 3, 5, 7]).unwrap();
    gate(&rt, &b, 99);
    let capacity_end = ordinary_end(rt, b, second);
    assert_eq!(capacity_end, completions[0]);

    // A new actor in the same slot cannot receive or install the old operation.
    let rt = Runtime::with_read_capacity(allowance, 1, 1);
    let old = actor(&rt);
    let (_old_writer, old_receipt) = install(&rt, &old);
    gate(&rt, &old, 99);
    wait(&old, |s| s.stage == 2 && s.waiting);
    old.kill();
    done(&old, 2);
    balanced(&old_receipt, false);
    let new = actor(&rt);
    assert_eq!(new.id().slot(), old.id().slot());
    assert_ne!(new.id(), old.id());
    let (read, _write) = UnixStream::pair().unwrap();
    assert!(matches!(
        rt.client().install_read(old.id(), read, 8),
        Err(ReadAdmissionError::NotLive)
    ));
    let (mut writer, new_receipt) = install(&rt, &new);
    writer.write_all(&[2, 3, 5, 7]).unwrap();
    gate(&rt, &new, 99);
    assert_eq!(ordinary_end(rt, new, new_receipt), completions[0]);

    let rt = Runtime::with_read_capacity(allowance, 2, 2);
    let client = rt.client();
    let a = actor(&rt);
    let b = actor(&rt);
    let (_wa, ra) = install(&rt, &a);
    let (_wb, rb) = install(&rt, &b);
    gate(&rt, &a, 99);
    gate(&rt, &b, 99);
    wait(&a, |s| s.stage == 2 && s.waiting);
    wait(&b, |s| s.stage == 2 && s.waiting);
    rt.shutdown();
    balanced(&ra, false);
    balanced(&rb, false);
    assert!(client.read_driver_stopped());
    assert_eq!(client.active_reads(), 0);
    done(&a, 2);
    done(&b, 2);

    let mut race_deliveries = 0;
    for _ in 0..16 {
        let rt = Runtime::with_read_capacity(allowance, 2, 1);
        let a = actor(&rt);
        let (mut writer, receipt) = install(&rt, &a);
        gate(&rt, &a, 99);
        wait(&a, |s| s.stage == 2 && s.waiting);
        let barrier = Arc::new(Barrier::new(3));
        let write_gate = barrier.clone();
        let kill_gate = barrier.clone();
        let kill = a.clone();
        let write_thread = std::thread::spawn(move || {
            write_gate.wait();
            match writer.write_all(&[2, 3, 5, 7]) {
                Ok(()) => {}
                Err(e) => assert!(matches!(
                    e.kind(),
                    std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                )),
            }
        });
        let kill_thread = std::thread::spawn(move || {
            kill_gate.wait();
            kill.kill();
        });
        barrier.wait();
        write_thread.join().unwrap();
        kill_thread.join().unwrap();
        let end = done(&a, 2);
        assert!(end.stage == 2 || end.stage == 3);
        let r = wait_read(&receipt, |s| s.retired && s.storage_drops == 1);
        balanced(&receipt, r.delivered);
        race_deliveries += r.delivered as u64;
        frozen_application(&end, &a.snapshot());
        rt.shutdown();
    }
    println!("{{\"kind\":\"read_controls\",\"passed\":true,\"speed_tests\":false,\"allowance\":{},\"completions\":{:?},\"stops\":{:?},\"eof_result\":0,\"unavailable_result\":90,\"capacity_rejected\":true,\"stale_isolated\":true,\"shutdown_settled\":2,\"peer_progress\":true,\"read_kill_races\":16,\"race_deliveries\":{}}}",allowance,completions,stops,race_deliveries);
}
