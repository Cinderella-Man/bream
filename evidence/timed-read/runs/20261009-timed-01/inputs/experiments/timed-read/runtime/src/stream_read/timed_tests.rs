use super::*;
use std::time::{Duration, Instant};

#[allow(unsafe_code)]
fn program() -> Program {
    unsafe {
        Program::__from_generated(|cx| {
            Box::pin(async move {
                let _ = cx.receive(None).await;
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
fn poll<T: Future + Unpin>(future: &mut T) -> Poll<T::Output> {
    let waker = Waker::from(Arc::new(NoopWake));
    Pin::new(future).poll(&mut Context::from_waker(&waker))
}
fn timeout(future: &mut ReadNextFor) {
    assert!(matches!(
        poll(future),
        Poll::Ready(TimedStreamResult::Timeout)
    ));
}
fn data(future: &mut ReadNextFor, expected: &[u8]) -> StreamBytes {
    let Poll::Ready(TimedStreamResult::Data(bytes)) = poll(future) else {
        panic!("expected timed data");
    };
    assert_eq!(bytes.bytes, expected);
    bytes
}
fn wait(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(20);
    while !condition() {
        assert!(Instant::now() < end, "correctness watchdog");
        std::thread::yield_now();
    }
}
struct Case {
    rt: Option<Runtime>,
    handle: ProcessHandle,
    hub: Arc<Hub>,
    _control: UnixStream,
    writer: Option<UnixStream>,
    receipt: StreamReceipt,
    stream: Arc<Stream>,
    clock: Arc<timer::Clock>,
}
impl Case {
    fn new() -> Self {
        let rt = Runtime::paused_with_capacity(4, 1);
        let handle = rt.spawn(program(), vec![7]).unwrap();
        let clock = rt.client().shared.clock.clone();
        let (hub, control) = Hub::new(2);
        let (socket, writer) = UnixStream::pair().unwrap();
        let receipt = hub.install(&handle.metrics, socket, 4).unwrap();
        let stream = handle.metrics.stream.lock().unwrap().clone().unwrap();
        Self {
            rt: Some(rt),
            handle,
            hub,
            _control: control,
            writer: Some(writer),
            receipt,
            stream,
            clock,
        }
    }
    fn read(&self, ticks: u64) -> ReadNextFor {
        context(&self.handle).read_next_for(ticks)
    }
    fn send(&mut self, bytes: &[u8]) {
        self.writer.as_mut().unwrap().write_all(bytes).unwrap();
    }
    fn expire(&self, now: u128) {
        self.clock.advance_to(now).unwrap();
        self.clock.service(8);
    }
    fn close(&self) {
        self.stream.close();
        self.hub.service();
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        self.close();
        if let Some(rt) = self.rt.take() {
            rt.shutdown();
        }
    }
}

#[test]
fn zero_attempts_once_without_timer_then_endpoint_and_cached_eof_reuse() {
    let mut c = Case::new();
    let mut empty = c.read(0);
    assert!(poll(&mut empty).is_pending());
    c.hub.service();
    timeout(&mut empty);
    assert_eq!(
        (
            c.receipt.snapshot().attempts,
            c.receipt.snapshot().timeouts,
            c.clock.snapshot().armed
        ),
        (1, 1, 0)
    );
    c.send(&[2, 3]);
    let mut next = c.read(0);
    c.hub.service();
    let bytes = data(&mut next, &[2, 3]);
    drop(c.writer.take());
    let mut eof = c.read(0);
    c.hub.service();
    assert!(matches!(
        poll(&mut eof),
        Poll::Ready(TimedStreamResult::Eof)
    ));
    let before = c.receipt.snapshot();
    let mut again = c.read(8);
    assert!(matches!(
        poll(&mut again),
        Poll::Ready(TimedStreamResult::Eof)
    ));
    assert_eq!(c.receipt.snapshot(), before);
    assert_eq!(bytes.read(1), 3);
    drop(bytes);
    c.hub.service();
    let s = c.receipt.snapshot();
    assert_eq!(
        (
            s.started,
            s.settled,
            s.delivered,
            s.discarded,
            s.drops,
            s.endpoint_drops
        ),
        (3, 3, 3, 0, 3, 1)
    );
    assert_eq!(c.clock.snapshot().armed, 0);
}
#[test]
fn pending_expiry_settles_before_reuse_without_consuming_later_bytes() {
    let mut c = Case::new();
    let mut first = c.read(5);
    c.hub.service();
    assert!(poll(&mut first).is_pending());
    c.expire(4);
    assert!(poll(&mut first).is_pending());
    c.expire(5);
    timeout(&mut first);
    let s = c.receipt.snapshot();
    assert_eq!(
        (
            s.started,
            s.settled,
            s.drops,
            s.live_bytes,
            s.endpoint_drops,
            s.attempts
        ),
        (1, 1, 1, 0, 0, 1)
    );
    c.send(&[5]);
    let mut next = c.read(7);
    c.hub.service();
    drop(data(&mut next, &[5]));
    drop(first);
    assert_eq!(c.receipt.snapshot().last_id, 2);
    assert_eq!(
        (
            c.clock.snapshot().armed,
            c.clock.snapshot().expired,
            c.clock.snapshot().cancelled,
            c.clock.snapshot().pending
        ),
        (2, 1, 1, 0)
    );
}
#[test]
fn completed_result_wins_detached_expiry_before_application_consumption() {
    let mut c = Case::new();
    let mut read = c.read(5);
    c.clock.advance_to(5).unwrap();
    let (key, target) = c.clock.take_due().unwrap();
    c.send(&[7]);
    c.hub.service();
    c.clock.expire(key, target.clone());
    c.clock.expire(key, target);
    drop(data(&mut read, &[7]));
    assert_eq!(
        (c.receipt.snapshot().timeouts, c.clock.snapshot().stale),
        (0, 2)
    );
}
#[test]
fn expiry_before_claim_suppresses_retained_readiness_ticket() {
    let mut c = Case::new();
    let mut read = c.read(5);
    c.hub.service();
    let ticket = c.stream.ticket().unwrap();
    c.send(&[11]);
    c.expire(5);
    assert!(!ticket.dispatch());
    timeout(&mut read);
    let mut next = c.read(0);
    assert!(!ticket.dispatch());
    c.hub.service();
    drop(data(&mut next, &[11]));
    assert_eq!(c.receipt.snapshot().attempts, 2);
}
#[test]
fn running_expiry_before_final_check_skips_read_and_waits_for_settlement() {
    let mut c = Case::new();
    let mut read = c.read(5);
    let id = read.inner.id.unwrap();
    let request = c.stream.claim(id).unwrap();
    c.expire(5);
    assert!(poll(&mut read).is_pending());
    let mut busy = c.read(8);
    assert!(matches!(
        poll(&mut busy),
        Poll::Ready(TimedStreamResult::Busy)
    ));
    assert_eq!(
        (
            c.receipt.snapshot().settled,
            c.receipt.snapshot().live_bytes,
            c.clock.snapshot().pending
        ),
        (0, 4, 0)
    );
    c.send(&[13]);
    let skip = request.signals.expired.load(Ordering::Acquire);
    assert!(skip);
    c.stream.finish(id, perform(request, skip));
    timeout(&mut read);
    let mut next = c.read(0);
    c.hub.service();
    drop(data(&mut next, &[13]));
    assert_eq!(
        (c.receipt.snapshot().attempts, c.receipt.snapshot().timeouts),
        (1, 1)
    );
}
#[test]
fn positive_claimed_read_wins_expiry_after_its_final_cancellation_check() {
    let mut c = Case::new();
    let mut read = c.read(5);
    let id = read.inner.id.unwrap();
    let request = c.stream.claim(id).unwrap();
    let skip = request.signals.expired.load(Ordering::Acquire);
    assert!(!skip);
    c.expire(5);
    assert!(poll(&mut read).is_pending());
    c.send(&[17]);
    c.stream.finish(id, perform(request, skip));
    drop(data(&mut read, &[17]));
    assert_eq!(
        (
            c.receipt.snapshot().timeouts,
            c.clock.snapshot().expired,
            c.clock.snapshot().pending
        ),
        (0, 1, 0)
    );
}
#[test]
fn would_block_claimed_read_after_expiry_returns_timeout_without_overlap() {
    let c = Case::new();
    let mut read = c.read(5);
    let id = read.inner.id.unwrap();
    let request = c.stream.claim(id).unwrap();
    c.expire(5);
    c.stream.finish(id, perform(request, false));
    timeout(&mut read);
    assert_eq!(
        (
            c.receipt.snapshot().attempts,
            c.receipt.snapshot().timeouts,
            c.receipt.snapshot().live_bytes
        ),
        (1, 1, 0)
    );
    let next = c.read(5);
    assert_eq!(next.inner.id, Some(2));
    drop(next);
    c.hub.service();
    assert_eq!(c.clock.snapshot().pending, 0);
}
#[test]
fn claimed_eof_and_injected_error_win_expiry_and_cache_terminal_state() {
    for error in [false, true] {
        let mut c = Case::new();
        let mut read = c.read(5);
        let id = read.inner.id.unwrap();
        let request = c.stream.claim(id).unwrap();
        c.expire(5);
        let outcome = if error {
            drop(request);
            Outcome::Failed
        } else {
            drop(c.writer.take());
            perform(request, false)
        };
        c.stream.finish(id, outcome);
        assert!(matches!(
            (error, poll(&mut read)),
            (false, Poll::Ready(TimedStreamResult::Eof))
                | (true, Poll::Ready(TimedStreamResult::Failed))
        ));
        let before = c.receipt.snapshot();
        let mut next = c.read(5);
        assert!(matches!(
            (error, poll(&mut next)),
            (false, Poll::Ready(TimedStreamResult::Eof))
                | (true, Poll::Ready(TimedStreamResult::Failed))
        ));
        assert_eq!(c.receipt.snapshot(), before);
        assert_eq!(before.timeouts, 0);
    }
}
#[test]
fn detached_old_timer_cannot_expire_new_request_on_same_process() {
    let mut c = Case::new();
    let mut first = c.read(5);
    c.clock.advance_to(5).unwrap();
    let (key, target) = c.clock.take_due().unwrap();
    c.send(&[19]);
    c.hub.service();
    let bytes = data(&mut first, &[19]);
    let mut next = c.read(20);
    let id = next.inner.id.unwrap();
    let request = c.stream.claim(id).unwrap();
    c.clock.expire(key, target.clone());
    c.clock.expire(key, target);
    drop(first);
    assert!(!request.signals.expired.load(Ordering::Acquire));
    assert_eq!(
        c.stream
            .state
            .lock()
            .unwrap()
            .active
            .as_ref()
            .unwrap()
            .timer
            .as_ref()
            .unwrap()
            .key
            .deadline,
        25
    );
    c.send(&[23]);
    c.stream.finish(id, perform(request, false));
    drop(data(&mut next, &[23]));
    assert_eq!(bytes.read(0), 19);
    drop(bytes);
    assert_eq!(
        (c.clock.snapshot().stale, c.clock.snapshot().pending),
        (2, 0)
    );
}
#[test]
fn interrupted_retries_and_unrelated_wakes_keep_original_deadline() {
    let c = Case::new();
    let mut read = c.read(9);
    let id = read.inner.id.unwrap();
    c.hub.service();
    for tick in [2, 4, 8] {
        c.clock.advance_to(tick).unwrap();
        // Inject Interrupted's retained-request path; no native errno claim.
        let request = c.stream.claim(id).unwrap();
        c.stream.finish(id, Outcome::Pending(request));
        mailbox::notify(&c.handle.metrics);
        assert!(poll(&mut read).is_pending());
        assert_eq!(
            c.stream
                .state
                .lock()
                .unwrap()
                .active
                .as_ref()
                .unwrap()
                .timer
                .as_ref()
                .unwrap()
                .key
                .deadline,
            9
        );
    }
    assert_eq!(c.clock.snapshot().armed, 1);
    c.expire(9);
    timeout(&mut read);
}
#[test]
fn receive_and_stream_timer_domains_do_not_collide_at_same_identity_and_deadline() {
    let c = Case::new();
    let mut read = c.read(5);
    c.handle.metrics.mailbox.lock().unwrap().timer =
        Some(c.clock.arm(&c.handle.metrics, 1, 5).unwrap());
    assert_eq!(c.clock.snapshot().pending, 2);
    c.expire(5);
    timeout(&mut read);
    assert!(c.handle.snapshot().timer_expired);
    assert_eq!(
        (c.clock.snapshot().expired, c.clock.snapshot().callbacks),
        (2, 2)
    );
}
#[test]
fn abandoned_pending_running_and_ready_timed_reads_remove_their_timer() {
    for mode in 0..3 {
        let mut c = Case::new();
        let read = c.read(5);
        let id = read.inner.id.unwrap();
        let claimed = if mode == 1 { c.stream.claim(id) } else { None };
        if mode == 2 {
            c.send(&[29]);
            c.hub.service();
        }
        drop(read);
        assert_eq!(c.clock.snapshot().pending, 0);
        if let Some(request) = claimed {
            c.stream.finish(id, perform(request, true));
        }
        c.hub.service();
        assert_eq!(
            (
                c.receipt.snapshot().settled,
                c.receipt.snapshot().discarded,
                c.receipt.snapshot().drops
            ),
            (1, 1, 1)
        );
        c.send(&[31]);
        let mut next = c.read(0);
        c.hub.service();
        drop(data(&mut next, &[31]));
    }
}
#[test]
fn invalid_duration_and_deadline_overflow_publish_no_request_or_timer() {
    let c = Case::new();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || c.read(u64::from(u32::MAX) + 1)
    ))
    .is_err());
    c.clock.advance_to(u128::MAX).unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.read(1))).is_err());
    assert_eq!(
        (
            c.receipt.snapshot().started,
            c.receipt.snapshot().buffers,
            c.clock.snapshot().armed
        ),
        (0, 0, 0)
    );
    let mut zero = c.read(0);
    c.hub.service();
    timeout(&mut zero);
    assert_eq!(c.receipt.snapshot().last_id, 1); // Overflow did not consume identity.
}
#[test]
fn infinity_uses_no_timer_and_missing_endpoint_does_not_register_one() {
    let c = Case::new();
    let mut forever = context(&c.handle).read_next();
    c.hub.service();
    c.expire(u128::MAX);
    assert!(poll(&mut forever).is_pending());
    assert_eq!(c.clock.snapshot().armed, 0);
    drop(forever);
    c.hub.service();
    let rt = Runtime::paused(4);
    let h = rt.spawn(program(), vec![]).unwrap();
    let mut missing = context(&h).read_next_for(5);
    assert!(matches!(
        poll(&mut missing),
        Poll::Ready(TimedStreamResult::Unavailable)
    ));
    assert_eq!(rt.client().clock_snapshot().armed, 0);
    rt.shutdown();
}
#[test]
fn old_timer_and_claimed_read_outlive_stop_without_touching_same_slot_replacement() {
    let mut c = Case::new();
    let mut read = c.read(5);
    let id = read.inner.id.unwrap();
    let request = c.stream.claim(id).unwrap();
    c.clock.advance_to(5).unwrap();
    let (key, target) = c.clock.take_due().unwrap();
    c.handle.kill();
    c.rt.as_ref().unwrap().start();
    wait(|| c.handle.snapshot().cleanup_event > 0);
    let replacement = c.rt.as_ref().unwrap().spawn(program(), vec![]).unwrap();
    assert_eq!(replacement.id().slot(), c.handle.id().slot());
    assert_ne!(replacement.id(), c.handle.id());
    let (socket, mut writer) = UnixStream::pair().unwrap();
    let receipt = c.hub.install(&replacement.metrics, socket, 4).unwrap();
    let mut next = context(&replacement).read_next_for(20);
    c.clock.expire(key, target);
    c.send(&[37]);
    c.stream.finish(id, perform(request, false));
    c.hub.service();
    assert!(matches!(
        poll(&mut read),
        Poll::Ready(TimedStreamResult::Unavailable)
    ));
    assert_eq!(
        (
            c.receipt.snapshot().discarded,
            c.receipt.snapshot().live_bytes,
            c.receipt.snapshot().endpoint_drops
        ),
        (1, 0, 1)
    );
    assert_eq!(
        (receipt.snapshot().delivered, receipt.snapshot().timeouts),
        (0, 0)
    );
    writer.write_all(&[41]).unwrap();
    let s = replacement.metrics.stream.lock().unwrap().clone().unwrap();
    s.ticket().unwrap().dispatch();
    drop(data(&mut next, &[41]));
    s.close();
    c.hub.service();
    assert_eq!(c.clock.snapshot().pending, 0);
}
#[test]
fn stream_timer_service_obeys_existing_eight_callback_allowance() {
    let rt = Runtime::paused_with_capacity(4, 12);
    let (hub, _control) = Hub::new(12);
    let mut reads = Vec::new();
    let mut peers = Vec::new();
    for _ in 0..12 {
        let h = rt.spawn(program(), vec![]).unwrap();
        let (socket, peer) = UnixStream::pair().unwrap();
        hub.install(&h.metrics, socket, 4).unwrap();
        peers.push(peer);
        reads.push(context(&h).read_next_for(5));
    }
    let clock = rt.client().shared.clock.clone();
    clock.advance_to(5).unwrap();
    assert_eq!(clock.service(8), 8);
    assert_eq!(clock.snapshot().pending, 4);
    assert_eq!(clock.service(8), 4);
    for read in &mut reads {
        timeout(read);
    }
    assert_eq!(
        (clock.snapshot().expired, clock.snapshot().service_batches),
        (12, 2)
    );
    hub.close();
    hub.service();
    assert_eq!(hub.active(), 0);
    rt.shutdown();
}

#[test]
fn owner_stop_overrides_timeout_intent_and_disposes_committed_timeout() {
    for running in [false, true] {
        let mut c = Case::new();
        let mut read = c.read(5);
        let id = read.inner.id.unwrap();
        let request = if running { c.stream.claim(id) } else { None };
        c.expire(5);
        c.handle.kill();
        c.rt.as_ref().unwrap().start();
        wait(|| c.handle.snapshot().cleanup_event > 0);
        if let Some(request) = request {
            c.send(&[47]);
            c.stream.finish(id, perform(request, false));
        }
        c.hub.service();
        assert!(matches!(
            poll(&mut read),
            Poll::Ready(TimedStreamResult::Unavailable)
        ));
        let s = c.receipt.snapshot();
        assert_eq!(
            (
                s.started,
                s.settled,
                s.delivered,
                s.discarded,
                s.drops,
                s.endpoint_drops
            ),
            (1, 1, 0, 1, 1, 1)
        );
        assert_eq!(s.timeouts, if running { 0 } else { 1 });
        assert_eq!(c.clock.snapshot().pending, 0);
    }
}
