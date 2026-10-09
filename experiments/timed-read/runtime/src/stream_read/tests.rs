use super::*;
use std::time::{Duration, Instant};
fn wait(mut p: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(20);
    while !p() {
        assert!(Instant::now() < end, "correctness watchdog");
        std::thread::yield_now();
    }
}
#[allow(unsafe_code)]
fn program() -> Program {
    unsafe {
        Program::__from_generated(|context| {
            Box::pin(async move {
                let _ = context.receive(None).await;
            })
        })
    }
}
fn context(h: &ProcessHandle) -> ProcessContext {
    ProcessContext {
        input: None,
        tag: 0,
        argument: 0,
        target: None,
        budget: Arc::new(Budget {
            remaining: AtomicU32::new(100),
        }),
        metrics: h.metrics.clone(),
    }
}
fn poll(f: &mut ReadNext) -> Poll<StreamResult> {
    let w = Waker::from(Arc::new(NoopWake));
    Pin::new(f).poll(&mut Context::from_waker(&w))
}
fn install(
    hub: &Arc<Hub>,
    h: &ProcessHandle,
    size: usize,
) -> (UnixStream, StreamReceipt, Arc<Stream>) {
    let (reader, writer) = UnixStream::pair().unwrap();
    let r = hub.install(&h.metrics, reader, size).unwrap();
    let s = h.metrics.stream.lock().unwrap().as_ref().unwrap().clone();
    (writer, r, s)
}
fn data(f: &mut ReadNext) -> StreamBytes {
    let Poll::Ready(StreamResult::Data(bytes)) = poll(f) else {
        panic!("expected bytes")
    };
    bytes
}
fn closed(r: &StreamReceipt, started: u64, delivered: u64) {
    let s = r.snapshot();
    assert_eq!(
        (s.started, s.settled, s.delivered, s.discarded),
        (started, started, delivered, started - delivered)
    );
    assert_eq!(
        (
            s.buffers,
            s.drops,
            s.live_bytes,
            s.endpoint_drops,
            s.active_id,
            s.retired
        ),
        (started, started, 0, 1, 0, true)
    );
}
fn close(hub: &Arc<Hub>, stream: &Arc<Stream>) {
    stream.close();
    hub.service();
}
#[test]
fn sequential_short_reads_preserve_remainder_and_old_owned_buffers() {
    let rt = Runtime::paused(4);
    let h = rt.spawn(program(), vec![]).unwrap();
    let (hub, _c) = Hub::new(1);
    let (mut writer, r, s) = install(&hub, &h, 4);
    writer.write_all(&[2, 3, 5, 7, 11, 13, 17]).unwrap();
    drop(writer);
    let mut first = context(&h).read_next();
    let id1 = first.id.unwrap();
    let backing = match &s.state.lock().unwrap().active.as_ref().unwrap().phase {
        Phase::Pending { bytes, .. } => bytes.bytes.as_ptr(),
        _ => panic!(),
    };
    hub.service();
    let a = data(&mut first);
    assert_eq!(a.bytes, [2, 3, 5, 7]);
    assert_eq!(a.bytes.as_ptr(), backing);
    assert_eq!(r.snapshot().endpoint_drops, 0);
    let mut second = context(&h).read_next();
    assert_ne!(second.id, Some(id1));
    drop(first);
    hub.service();
    let b = data(&mut second);
    assert_eq!(b.bytes, [11, 13, 17]);
    assert_eq!(
        (r.snapshot().live_bytes, r.snapshot().endpoint_drops),
        (8, 0)
    );
    assert_eq!(a.read(0), 2);
    drop(a);
    assert_eq!(r.snapshot().live_bytes, 4);
    drop(b);
    let mut eof = context(&h).read_next();
    hub.service();
    assert!(matches!(poll(&mut eof), Poll::Ready(StreamResult::Eof)));
    hub.service();
    let before = r.snapshot();
    let mut again = context(&h).read_next();
    assert!(matches!(poll(&mut again), Poll::Ready(StreamResult::Eof)));
    assert_eq!(r.snapshot(), before);
    closed(&r, 3, 3);
    rt.shutdown();
}
#[test]
fn stale_cancel_completion_and_readiness_cannot_touch_new_running_request() {
    let rt = Runtime::paused(4);
    let h = rt.spawn(program(), vec![]).unwrap();
    let (hub, _c) = Hub::new(1);
    let (mut writer, r, s) = install(&hub, &h, 4);
    let mut first = context(&h).read_next();
    let id1 = first.id.unwrap();
    let old_ticket = s.ticket().unwrap();
    writer.write_all(&[3]).unwrap();
    hub.service();
    drop(data(&mut first));
    let mut second = context(&h).read_next();
    let id2 = second.id.unwrap();
    let request = s.claim(id2).unwrap();
    let before = r.snapshot();
    assert!(!s.cancel_request(id1));
    assert!(!old_ticket.dispatch());
    s.finish(id1, Outcome::Eof);
    let ghost = Arc::new(Counters::default());
    ghost.live_bytes.store(1, Ordering::Relaxed);
    let (ghost_socket, _ghost_peer) = UnixStream::pair().unwrap();
    s.finish(
        id1,
        Outcome::Data(
            Endpoint {
                socket: ghost_socket,
                counters: ghost.clone(),
            },
            StreamBytes {
                bytes: vec![99],
                counters: ghost.clone(),
            },
        ),
    );
    assert_eq!(
        (
            ghost.live_bytes.load(Ordering::Acquire),
            ghost.drops.load(Ordering::Acquire),
            ghost.endpoint_drops.load(Ordering::Acquire)
        ),
        (0, 1, 1)
    );
    drop(first);
    assert_eq!(r.snapshot(), before);
    writer.write_all(&[19]).unwrap();
    s.finish(id2, perform(request, false));
    let bytes = data(&mut second);
    assert_eq!(bytes.bytes, [19]);
    drop(bytes);
    close(&hub, &s);
    closed(&r, 2, 2);
    rt.shutdown();
}
#[test]
fn cancelled_running_request_stays_busy_until_it_settles() {
    let rt = Runtime::paused(4);
    let h = rt.spawn(program(), vec![]).unwrap();
    let (hub, _c) = Hub::new(1);
    let (mut writer, r, s) = install(&hub, &h, 4);
    let mut first = context(&h).read_next();
    let id1 = first.id.unwrap();
    let request = s.claim(id1).unwrap();
    assert!(s.cancel_request(id1));
    hub.service();
    let mut busy = context(&h).read_next();
    assert!(matches!(poll(&mut busy), Poll::Ready(StreamResult::Busy)));
    assert_eq!(
        (
            r.snapshot().started,
            r.snapshot().settled,
            r.snapshot().live_bytes
        ),
        (1, 0, 4)
    );
    s.finish(id1, perform(request, true));
    assert!(matches!(
        poll(&mut first),
        Poll::Ready(StreamResult::Unavailable)
    ));
    let mut second = context(&h).read_next();
    assert_ne!(second.id, Some(id1));
    writer.write_all(&[23]).unwrap();
    hub.service();
    let bytes = data(&mut second);
    assert_eq!(bytes.read(0), 23);
    drop(bytes);
    close(&hub, &s);
    closed(&r, 2, 1);
    rt.shutdown();
}
#[test]
fn pending_future_drop_cancels_only_its_request_and_keeps_endpoint() {
    let rt = Runtime::paused(4);
    let h = rt.spawn(program(), vec![]).unwrap();
    let (hub, _c) = Hub::new(1);
    let (mut writer, r, s) = install(&hub, &h, 4);
    let mut first = context(&h).read_next();
    assert!(poll(&mut first).is_pending());
    let id = first.id.unwrap();
    drop(first);
    hub.service();
    assert_eq!(
        (
            r.snapshot().attempts,
            r.snapshot().endpoint_drops,
            r.snapshot().discarded
        ),
        (0, 0, 1)
    );
    let mut second = context(&h).read_next();
    assert!(!s.cancel_request(id));
    writer.write_all(&[29]).unwrap();
    hub.service();
    drop(data(&mut second));
    close(&hub, &s);
    closed(&r, 2, 1);
    rt.shutdown();
}
#[test]
fn ready_result_drop_cancels_buffer_without_closing_endpoint() {
    let rt = Runtime::paused(4);
    let h = rt.spawn(program(), vec![]).unwrap();
    let (hub, _c) = Hub::new(1);
    let (mut writer, r, s) = install(&hub, &h, 4);
    let first = context(&h).read_next();
    writer.write_all(&[31]).unwrap();
    hub.service();
    assert_eq!((r.snapshot().settled, r.snapshot().live_bytes), (1, 4));
    drop(first);
    hub.service();
    assert_eq!(
        (
            r.snapshot().discarded,
            r.snapshot().endpoint_drops,
            r.snapshot().live_bytes
        ),
        (1, 0, 0)
    );
    let mut second = context(&h).read_next();
    writer.write_all(&[37]).unwrap();
    hub.service();
    drop(data(&mut second));
    close(&hub, &s);
    closed(&r, 2, 1);
    rt.shutdown();
}
#[test]
fn idle_endpoint_closes_while_previously_delivered_bytes_survive() {
    let rt = Runtime::paused(4);
    let h = rt.spawn(program(), vec![1]).unwrap();
    let (hub, _c) = Hub::new(1);
    let (mut writer, r, s) = install(&hub, &h, 4);
    let mut f = context(&h).read_next();
    writer.write_all(&[41]).unwrap();
    hub.service();
    let bytes = data(&mut f);
    close(&hub, &s);
    assert_eq!(
        (
            r.snapshot().retired,
            r.snapshot().endpoint_drops,
            r.snapshot().live_bytes,
            r.snapshot().drops
        ),
        (true, 1, 4, 0)
    );
    assert_eq!(bytes.read(0), 41);
    drop(bytes);
    closed(&r, 1, 1);
    rt.shutdown();
}
#[test]
fn claimed_old_read_survives_actor_cleanup_and_same_slot_replacement() {
    let rt = Runtime::paused_with_capacity(4, 1);
    let old = rt.spawn(program(), vec![7]).unwrap();
    let (hub, _c) = Hub::new(2);
    let (mut writer, r, s) = install(&hub, &old, 4);
    let mut first = context(&old).read_next();
    let id = first.id.unwrap();
    let request = s.claim(id).unwrap();
    writer.write_all(&[43]).unwrap();
    old.kill();
    rt.start();
    wait(|| old.snapshot().cleanup_event > 0);
    hub.service();
    assert_eq!(
        (
            old.snapshot().live_bytes,
            r.snapshot().live_bytes,
            r.snapshot().endpoint_drops,
            r.snapshot().settled
        ),
        (0, 4, 0, 0)
    );
    let new = rt.spawn(program(), vec![]).unwrap();
    assert_eq!(new.id().slot(), old.id().slot());
    assert_ne!(new.id(), old.id());
    wait(|| new.snapshot().parks > 0);
    let (_peer, new_r, new_s) = install(&hub, &new, 4);
    let before = new_r.snapshot();
    s.finish(id, perform(request, false));
    hub.service();
    closed(&r, 1, 0);
    assert_eq!(new_r.snapshot(), before);
    assert!(matches!(
        poll(&mut first),
        Poll::Ready(StreamResult::Unavailable)
    ));
    close(&hub, &new_s);
    closed(&new_r, 0, 0);
    rt.shutdown();
}
#[test]
fn failed_request_closes_and_caches_failure_without_more_buffers() {
    let rt = Runtime::paused(4);
    let h = rt.spawn(program(), vec![]).unwrap();
    let (hub, _c) = Hub::new(1);
    let (_writer, r, s) = install(&hub, &h, 4);
    let mut first = context(&h).read_next();
    let id = first.id.unwrap();
    drop(s.claim(id).unwrap());
    s.finish(id, Outcome::Failed);
    assert!(matches!(
        poll(&mut first),
        Poll::Ready(StreamResult::Failed)
    ));
    hub.service();
    let before = r.snapshot();
    let mut again = context(&h).read_next();
    assert!(matches!(
        poll(&mut again),
        Poll::Ready(StreamResult::Failed)
    ));
    assert_eq!(r.snapshot(), before);
    closed(&r, 1, 1);
    rt.shutdown();
}
#[test]
fn request_identity_exhaustion_closes_without_wrapping() {
    let rt = Runtime::paused(4);
    let h = rt.spawn(program(), vec![]).unwrap();
    let (hub, _c) = Hub::new(1);
    let (mut writer, r, s) = install(&hub, &h, 4);
    s.state.lock().unwrap().next = Some(u64::MAX);
    let mut f = context(&h).read_next();
    assert_eq!(f.id, Some(u64::MAX));
    writer.write_all(&[47]).unwrap();
    hub.service();
    drop(data(&mut f));
    let mut exhausted = context(&h).read_next();
    assert!(matches!(
        poll(&mut exhausted),
        Poll::Ready(StreamResult::Unavailable)
    ));
    hub.service();
    assert_eq!(r.snapshot().last_id, u64::MAX);
    closed(&r, 1, 1);
    rt.shutdown();
}
#[test]
fn idle_pending_ready_and_consumed_endpoints_all_retain_capacity() {
    let rt = Runtime::paused(4);
    let a = rt.spawn(program(), vec![]).unwrap();
    let b = rt.spawn(program(), vec![]).unwrap();
    let (hub, _c) = Hub::new(1);
    let (mut writer, r, s) = install(&hub, &a, 4);
    let reject = || {
        let (reader, mut peer) = UnixStream::pair().unwrap();
        peer.set_nonblocking(true).unwrap();
        assert!(matches!(
            hub.install(&b.metrics, reader, 4),
            Err(ReadAdmissionError::Capacity)
        ));
        assert_eq!(peer.read(&mut [0]).unwrap(), 0);
    };
    reject();
    let mut f = context(&a).read_next();
    reject();
    writer.write_all(&[53]).unwrap();
    hub.service();
    reject();
    drop(data(&mut f));
    reject();
    close(&hub, &s);
    closed(&r, 1, 1);
    let (_writer, next, stream) = install(&hub, &b, 4);
    close(&hub, &stream);
    closed(&next, 0, 0);
    rt.shutdown();
}
#[test]
fn invalid_size_duplicate_installation_and_closed_hub_are_rejected() {
    let rt = Runtime::paused(4);
    let h = rt.spawn(program(), vec![]).unwrap();
    let (hub, _c) = Hub::new(1);
    for size in [0, 4097] {
        let (reader, _writer) = UnixStream::pair().unwrap();
        assert!(matches!(
            hub.install(&h.metrics, reader, size),
            Err(ReadAdmissionError::Size)
        ));
    }
    let (_writer, r, s) = install(&hub, &h, 4096);
    let (reader, _writer) = UnixStream::pair().unwrap();
    assert!(matches!(
        hub.install(&h.metrics, reader, 4),
        Err(ReadAdmissionError::AlreadyInstalled)
    ));
    close(&hub, &s);
    closed(&r, 0, 0);
    let other = rt.spawn(program(), vec![]).unwrap();
    hub.close();
    let (reader, _writer) = UnixStream::pair().unwrap();
    assert!(matches!(
        hub.install(&other.metrics, reader, 4),
        Err(ReadAdmissionError::Closed)
    ));
    rt.shutdown();
}
#[test]
fn read_without_installed_stream_is_unavailable() {
    let rt = Runtime::paused(4);
    let h = rt.spawn(program(), vec![]).unwrap();
    let mut f = context(&h).read_next();
    assert!(matches!(
        poll(&mut f),
        Poll::Ready(StreamResult::Unavailable)
    ));
    rt.shutdown();
}
#[test]
fn trusted_installation_rejects_fatal_stale_foreign_and_closed_owner() {
    let rt = Runtime::with_stream_capacity(4, 1, 1);
    let client = rt.client();
    let old = rt.spawn(program(), vec![]).unwrap();
    let other = Runtime::paused(4);
    let foreign = other.spawn(program(), vec![]).unwrap();
    old.kill();
    for id in [old.id(), foreign.id()] {
        let (reader, _writer) = UnixStream::pair().unwrap();
        assert!(matches!(
            client.install_stream(id, reader, 4),
            Err(ReadAdmissionError::NotLive)
        ));
    }
    wait(|| old.snapshot().cleanup_event > 0);
    let new = rt.spawn(program(), vec![]).unwrap();
    assert_eq!(new.id().slot(), old.id().slot());
    let (reader, _writer) = UnixStream::pair().unwrap();
    assert!(matches!(
        client.install_stream(old.id(), reader, 4),
        Err(ReadAdmissionError::NotLive)
    ));
    rt.shutdown();
    let (reader, _writer) = UnixStream::pair().unwrap();
    assert!(matches!(
        client.install_stream(new.id(), reader, 4),
        Err(ReadAdmissionError::Closed)
    ));
    assert!(client.stream_driver_stopped());
    other.shutdown();
}
#[test]
fn shutdown_settles_idle_pending_and_ready_with_open_remote_peers() {
    let rt = Runtime::with_stream_capacity(4, 3, 3);
    let client = rt.client();
    let mut receipts = Vec::new();
    let mut peers = Vec::new();
    let mut handles = Vec::new();
    for _ in 0..3 {
        let h = rt.spawn(program(), vec![]).unwrap();
        let (reader, writer) = UnixStream::pair().unwrap();
        receipts.push(client.install_stream(h.id(), reader, 4).unwrap());
        peers.push(writer);
        handles.push(h);
    }
    let _pending = context(&handles[1]).read_next();
    let _ready = context(&handles[2]).read_next();
    peers[2].write_all(&[59]).unwrap();
    wait(|| receipts[2].snapshot().settled == 1);
    rt.shutdown();
    closed(&receipts[0], 0, 0);
    closed(&receipts[1], 1, 0);
    closed(&receipts[2], 1, 0);
    assert!(client.stream_driver_stopped());
    assert_eq!(client.active_streams(), 0);
}
#[test]
fn control_failure_closes_idle_and_pending_endpoints_and_wakes_waiter() {
    let rt = Runtime::paused(4);
    let a = rt.spawn(program(), vec![]).unwrap();
    let b = rt.spawn(program(), vec![]).unwrap();
    let (hub, control) = Hub::new(2);
    let (_wa, ra, _sa) = install(&hub, &a, 4);
    let (_wb, rb, _sb) = install(&hub, &b, 4);
    let mut f = context(&a).read_next();
    assert!(poll(&mut f).is_pending());
    let driverhub = hub.clone();
    let driver = std::thread::spawn(move || driverhub.run(control));
    hub.signal.as_ref().unwrap().lock().unwrap().take();
    wait(|| hub.stopped());
    assert!(matches!(
        poll(&mut f),
        Poll::Ready(StreamResult::Unavailable)
    ));
    closed(&ra, 1, 0);
    closed(&rb, 0, 0);
    driver.join().unwrap();
    rt.shutdown();
}
#[test]
fn cancellation_after_claim_is_observed_before_attempt() {
    let rt = Runtime::paused(4);
    let h = rt.spawn(program(), vec![]).unwrap();
    let (hub, _c) = Hub::new(1);
    let (mut writer, r, s) = install(&hub, &h, 4);
    let mut f = context(&h).read_next();
    let id = f.id.unwrap();
    let request = s.claim(id).unwrap();
    writer.write_all(&[61]).unwrap();
    s.cancel_request(id);
    let cancelled = request.signals.cancelled.load(Ordering::Acquire);
    s.finish(id, perform(request, cancelled));
    assert!(matches!(
        poll(&mut f),
        Poll::Ready(StreamResult::Unavailable)
    ));
    assert_eq!(r.snapshot().attempts, 0);
    close(&hub, &s);
    closed(&r, 1, 0);
    rt.shutdown();
}
#[test]
fn actor_stop_retains_the_captured_snapshot_through_a_real_poll_call() {
    let rt = Runtime::paused(4);
    let h = rt.spawn(program(), vec![]).unwrap();
    let (hub, control) = Hub::new(1);
    let (mut peer, r, _s) = install(&hub, &h, 4);
    let mut f = context(&h).read_next();
    assert!(poll(&mut f).is_pending());
    let (captured_tx, captured_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    *hub.poll_gate.lock().unwrap() = Some(PollGate {
        snapshot: captured_tx,
        resume: resume_rx,
        result: result_tx,
    });
    let driver_hub = hub.clone();
    let driver = std::thread::spawn(move || driver_hub.run(control));
    captured_rx.recv_timeout(Duration::from_secs(20)).unwrap();
    h.kill();
    rt.start();
    wait(|| h.snapshot().cleanup_event > 0);
    assert_eq!(
        (
            r.snapshot().retired,
            r.snapshot().endpoint_drops,
            r.snapshot().live_bytes
        ),
        (false, 0, 4)
    );
    peer.write_all(&[67]).unwrap(); // The captured endpoint remains physically open.
    resume_tx.send(()).unwrap();
    let events = result_rx.recv_timeout(Duration::from_secs(20)).unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|event| event & libc::POLLNVAL == 0));
    wait(|| r.snapshot().retired);
    closed(&r, 1, 0);
    assert_eq!(r.snapshot().attempts, 1);
    assert!(matches!(
        poll(&mut f),
        Poll::Ready(StreamResult::Unavailable)
    ));
    hub.close();
    driver.join().unwrap();
    rt.shutdown();
}
