use super::*;
use std::time::{Duration, Instant};

fn wait(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !predicate() {
        assert!(Instant::now() < deadline, "correctness watchdog");
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
fn context(handle: &ProcessHandle) -> ProcessContext {
    ProcessContext {
        input: None,
        tag: 0,
        argument: 0,
        target: None,
        budget: Arc::new(Budget {
            remaining: AtomicU32::new(100),
        }),
        metrics: handle.metrics.clone(),
    }
}
fn poll(future: &mut ReadInput) -> Poll<ReadResult> {
    let waker = Waker::from(Arc::new(NoopWake));
    Pin::new(future).poll(&mut Context::from_waker(&waker))
}
fn installed(
    hub: &Arc<Hub>,
    actor: &ProcessHandle,
    size: usize,
) -> (UnixStream, ReadReceipt, Arc<Operation>) {
    let (reader, writer) = UnixStream::pair().unwrap();
    let receipt = hub.install(&actor.metrics, reader, size).unwrap();
    let op = actor
        .metrics
        .read_operation
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone();
    (writer, receipt, op)
}
fn settled(receipt: &ReadReceipt, delivered: bool) {
    let s = receipt.snapshot();
    assert!(s.settled && s.retired);
    assert_eq!(
        (s.delivered, s.live_bytes, s.storage_drops, s.endpoint_drops),
        (delivered, 0, 1, 1)
    );
}

#[test]
fn pending_then_short_read_with_hup_retains_permit_until_consumed() {
    let rt = Runtime::paused(4);
    let actor = rt.spawn(program(), vec![]).unwrap();
    let (hub, _control) = Hub::new(1);
    let (mut writer, receipt, _op) = installed(&hub, &actor, 8);
    let backing = match &*actor
        .metrics
        .read_operation
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .state
        .lock()
        .unwrap()
    {
        Phase::Pending(request) => request.bytes.bytes.as_ptr(),
        _ => panic!("pending buffer"),
    };
    let mut future = context(&actor).read_input();
    assert!(poll(&mut future).is_pending());
    hub.service();
    assert_eq!(
        (receipt.snapshot().attempts, receipt.snapshot().live_bytes),
        (1, 8)
    );
    writer.write_all(&[7, 11]).unwrap();
    drop(writer);
    hub.service();
    assert!(receipt.snapshot().settled);
    assert_eq!(hub.active(), 1);
    assert!(!actor.metrics.mailbox.lock().unwrap().waiting);
    let Poll::Ready(ReadResult::Data(bytes)) = poll(&mut future) else {
        panic!("data")
    };
    assert_eq!((bytes.len(), bytes.read(0), bytes.read(1)), (2, 7, 11));
    assert_eq!(bytes.bytes.as_ptr(), backing);
    hub.service();
    assert_eq!(hub.active(), 0);
    assert_eq!(
        (
            receipt.snapshot().live_bytes,
            receipt.snapshot().storage_drops
        ),
        (8, 0)
    );
    drop(bytes);
    settled(&receipt, true);
    rt.shutdown();
}
#[test]
fn eof_drops_storage_and_duplicate_consumer_is_unavailable() {
    let rt = Runtime::paused(4);
    let actor = rt.spawn(program(), vec![]).unwrap();
    let (hub, _control) = Hub::new(1);
    let (writer, receipt, _op) = installed(&hub, &actor, 4);
    drop(writer);
    let mut first = context(&actor).read_input();
    let mut second = context(&actor).read_input();
    assert!(matches!(
        poll(&mut second),
        Poll::Ready(ReadResult::Unavailable)
    ));
    hub.service();
    assert!(matches!(poll(&mut first), Poll::Ready(ReadResult::Eof)));
    hub.service();
    settled(&receipt, true);
    rt.shutdown();
}
#[test]
fn cancel_before_claim_performs_no_read() {
    let rt = Runtime::paused(4);
    let actor = rt.spawn(program(), vec![]).unwrap();
    let (hub, _control) = Hub::new(1);
    let (mut writer, receipt, op) = installed(&hub, &actor, 4);
    writer.write_all(&[9]).unwrap();
    op.cancel();
    hub.service();
    settled(&receipt, false);
    assert_eq!(receipt.snapshot().attempts, 0);
    rt.shutdown();
}
#[test]
fn claimed_request_survives_actor_stop_slot_reuse_and_late_completion() {
    let rt = Runtime::paused_with_capacity(4, 1);
    let actor = rt.spawn(program(), vec![1]).unwrap();
    let (hub, _control) = Hub::new(1);
    let (mut writer, receipt, op) = installed(&hub, &actor, 8);
    let request = op.claim().unwrap();
    writer.write_all(&[3, 5]).unwrap();
    actor.kill();
    rt.start();
    wait(|| actor.snapshot().cleanup_event > 0);
    assert_eq!(
        (
            receipt.snapshot().settled,
            receipt.snapshot().live_bytes,
            receipt.snapshot().storage_drops
        ),
        (false, 8, 0)
    );
    let replacement = rt.spawn(program(), vec![]).unwrap();
    assert_eq!(actor.id().slot(), replacement.id().slot());
    assert_ne!(actor.id(), replacement.id());
    wait(|| replacement.snapshot().parks > 0);
    let before = replacement.snapshot();
    // Model the driver having passed its cancellation check before stop.
    op.finish(attempt(request, false));
    hub.service();
    settled(&receipt, false);
    assert_eq!(receipt.snapshot().attempts, 1);
    assert_eq!(replacement.snapshot(), before);
    assert_eq!(actor.snapshot().terminal, 2);
    rt.shutdown();
}
#[test]
fn ready_result_cancelled_before_consumption_is_dropped_once() {
    let rt = Runtime::paused(4);
    let actor = rt.spawn(program(), vec![]).unwrap();
    let (hub, _control) = Hub::new(1);
    let (mut writer, receipt, op) = installed(&hub, &actor, 4);
    writer.write_all(&[13]).unwrap();
    hub.service();
    assert!(receipt.snapshot().settled);
    assert_eq!(receipt.snapshot().live_bytes, 4);
    op.cancel();
    hub.service();
    settled(&receipt, false);
    let mut future = context(&actor).read_input();
    assert!(matches!(
        poll(&mut future),
        Poll::Ready(ReadResult::Unavailable)
    ));
    rt.shutdown();
}
#[test]
fn duplicate_completion_cannot_overwrite_the_first_result() {
    let rt = Runtime::paused(4);
    let actor = rt.spawn(program(), vec![]).unwrap();
    let (hub, _control) = Hub::new(1);
    let (mut writer, receipt, op) = installed(&hub, &actor, 4);
    writer.write_all(&[17]).unwrap();
    hub.service();
    let duplicate = Arc::new(Counters::default());
    duplicate.live_bytes.store(1, Ordering::Relaxed);
    op.finish(Attempt::Done(ReadResult::Data(ReadBytes {
        bytes: vec![99],
        counters: duplicate.clone(),
    })));
    assert_eq!(duplicate.storage_drops.load(Ordering::Acquire), 1);
    let mut future = context(&actor).read_input();
    let Poll::Ready(ReadResult::Data(bytes)) = poll(&mut future) else {
        panic!("first result")
    };
    assert_eq!(bytes.read(0), 17);
    drop(bytes);
    hub.service();
    settled(&receipt, true);
    rt.shutdown();
}
#[test]
fn ready_unconsumed_results_hold_admission_and_rejections_close_endpoint() {
    let rt = Runtime::paused(4);
    let a = rt.spawn(program(), vec![]).unwrap();
    let b = rt.spawn(program(), vec![]).unwrap();
    let (hub, _control) = Hub::new(1);
    let (mut writer, receipt, op) = installed(&hub, &a, 4);
    writer.write_all(&[1]).unwrap();
    hub.service();
    let (reader, mut peer) = UnixStream::pair().unwrap();
    peer.set_nonblocking(true).unwrap();
    assert!(matches!(
        hub.install(&b.metrics, reader, 4),
        Err(ReadAdmissionError::Capacity)
    ));
    assert_eq!(peer.read(&mut [0]).unwrap(), 0);
    op.cancel();
    hub.service();
    settled(&receipt, false);
    let (_writer, next, _op) = installed(&hub, &b, 4);
    hub.close();
    hub.service();
    settled(&next, false);
    rt.shutdown();
}
#[test]
fn admission_limits_and_one_installation_are_enforced() {
    let rt = Runtime::paused(4);
    let actor = rt.spawn(program(), vec![]).unwrap();
    let (hub, _control) = Hub::new(1);
    for size in [0, 4097] {
        let (reader, _writer) = UnixStream::pair().unwrap();
        assert!(matches!(
            hub.install(&actor.metrics, reader, size),
            Err(ReadAdmissionError::Size)
        ));
    }
    let (_writer, receipt, _op) = installed(&hub, &actor, 4096);
    let (reader, _writer) = UnixStream::pair().unwrap();
    assert!(matches!(
        hub.install(&actor.metrics, reader, 4),
        Err(ReadAdmissionError::AlreadyInstalled)
    ));
    hub.close();
    hub.service();
    settled(&receipt, false);
    rt.shutdown();
}
#[test]
fn dropping_pending_wait_cancels_without_dropping_claimed_storage() {
    let rt = Runtime::paused(4);
    let actor = rt.spawn(program(), vec![]).unwrap();
    let (hub, _control) = Hub::new(1);
    let (_writer, receipt, op) = installed(&hub, &actor, 4);
    let mut future = context(&actor).read_input();
    assert!(poll(&mut future).is_pending());
    let request = op.claim().unwrap();
    drop(future);
    assert!(receipt.snapshot().cancelled);
    assert_eq!(receipt.snapshot().live_bytes, 4);
    op.finish(attempt(request, true));
    hub.service();
    settled(&receipt, false);
    rt.shutdown();
}
#[test]
fn failed_completion_is_typed_and_releases_request_storage() {
    let rt = Runtime::paused(4);
    let actor = rt.spawn(program(), vec![]).unwrap();
    let (hub, _control) = Hub::new(1);
    let (_writer, receipt, op) = installed(&hub, &actor, 4);
    drop(op.claim().unwrap());
    op.finish(Attempt::Done(ReadResult::Failed));
    let mut future = context(&actor).read_input();
    assert!(matches!(poll(&mut future), Poll::Ready(ReadResult::Failed)));
    hub.service();
    settled(&receipt, true);
    rt.shutdown();
}
#[test]
fn trusted_admission_rejects_killed_stale_foreign_and_closed_origins() {
    let rt = Runtime::with_read_capacity(4, 1, 1);
    let client = rt.client();
    let actor = rt.spawn(program(), vec![]).unwrap();
    let other = Runtime::paused(4);
    let foreign = other.spawn(program(), vec![]).unwrap();
    actor.kill();
    for id in [actor.id(), foreign.id()] {
        let (reader, _writer) = UnixStream::pair().unwrap();
        assert!(matches!(
            client.install_read(id, reader, 4),
            Err(ReadAdmissionError::NotLive)
        ));
    }
    wait(|| actor.snapshot().cleanup_event > 0);
    let replacement = rt.spawn(program(), vec![]).unwrap();
    assert_eq!(replacement.id().slot(), actor.id().slot());
    let (reader, _writer) = UnixStream::pair().unwrap();
    assert!(matches!(
        client.install_read(actor.id(), reader, 4),
        Err(ReadAdmissionError::NotLive)
    ));
    rt.shutdown();
    assert!(client.read_driver_stopped());
    assert_eq!(client.active_reads(), 0);
    let (reader, _writer) = UnixStream::pair().unwrap();
    assert!(matches!(
        client.install_read(replacement.id(), reader, 4),
        Err(ReadAdmissionError::Closed)
    ));
    other.shutdown();
}
#[test]
fn driver_shutdown_settles_pending_and_ready_with_remote_peers_open() {
    let rt = Runtime::with_read_capacity(4, 2, 2);
    let client = rt.client();
    let a = rt.spawn(program(), vec![]).unwrap();
    let b = rt.spawn(program(), vec![]).unwrap();
    let (ra, mut wa) = UnixStream::pair().unwrap();
    let (rb, _wb) = UnixStream::pair().unwrap();
    let first = client.install_read(a.id(), ra, 8).unwrap();
    let second = client.install_read(b.id(), rb, 8).unwrap();
    wa.write_all(&[23]).unwrap();
    wait(|| first.snapshot().settled && second.snapshot().attempts > 0);
    rt.shutdown();
    assert!(client.read_driver_stopped());
    assert_eq!(client.active_reads(), 0);
    settled(&first, false);
    settled(&second, false);
    assert_eq!(a.snapshot().terminal, 2);
    assert_eq!(b.snapshot().terminal, 2);
}

#[test]
fn cancellation_observed_after_claim_suppresses_the_read_attempt() {
    let rt = Runtime::paused(4);
    let actor = rt.spawn(program(), vec![]).unwrap();
    let (hub, _control) = Hub::new(1);
    let (mut writer, receipt, op) = installed(&hub, &actor, 4);
    writer.write_all(&[9]).unwrap();
    let request = op.claim().unwrap();
    op.cancel();
    op.finish(attempt(
        request,
        op.counters.cancelled.load(Ordering::Acquire),
    ));
    hub.service();
    settled(&receipt, false);
    assert_eq!(receipt.snapshot().attempts, 0);
    rt.shutdown();
}
#[test]
fn broken_control_channel_fails_closed_and_wakes_the_pending_waiter() {
    let rt = Runtime::paused(4);
    let actor = rt.spawn(program(), vec![]).unwrap();
    let (hub, control) = Hub::new(1);
    let (_writer, receipt, _op) = installed(&hub, &actor, 8);
    let worker_hub = hub.clone();
    let driver = std::thread::spawn(move || worker_hub.run(control));
    let mut future = context(&actor).read_input();
    assert!(poll(&mut future).is_pending());
    hub.signal.as_ref().unwrap().lock().unwrap().take();
    wait(|| hub.stopped());
    settled(&receipt, false);
    assert!(!actor.metrics.mailbox.lock().unwrap().waiting);
    assert!(matches!(
        poll(&mut future),
        Poll::Ready(ReadResult::Unavailable)
    ));
    let other = rt.spawn(program(), vec![]).unwrap();
    let (reader, _writer) = UnixStream::pair().unwrap();
    assert!(matches!(
        hub.install(&other.metrics, reader, 8),
        Err(ReadAdmissionError::Closed)
    ));
    driver.join().unwrap();
    rt.shutdown();
}
