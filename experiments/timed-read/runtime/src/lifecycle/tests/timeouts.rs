use super::*;

struct Case {
    client: RuntimeClient,
    receiver: mpsc::Receiver<Command>,
    handle: ProcessHandle,
    task: Option<Task>,
}
impl Case {
    fn new() -> Self {
        let (client, receiver) = fixture();
        let handle = client.publish(prepared()).unwrap();
        let task = published(&receiver);
        task.metrics
            .runtime
            .set(Arc::downgrade(&client.shared))
            .unwrap();
        Self {
            client,
            receiver,
            handle,
            task: Some(task),
        }
    }
    fn timed(&self, tag: u64, ticks: u64) -> TimedReceive {
        let task = self.task.as_ref().unwrap();
        TimedReceive::new(task.metrics.clone(), task.budget.clone(), Some(tag), ticks)
    }
    fn poll(&self, receive: &mut TimedReceive, budget: u32) -> Poll<ReceiveResult> {
        self.task
            .as_ref()
            .unwrap()
            .budget
            .remaining
            .store(budget, Ordering::Relaxed);
        let waker = Waker::from(Arc::new(NoopWake));
        Pin::new(receive).poll(&mut Context::from_waker(&waker))
    }
    fn send(&self, tag: u64, value: u64) -> MessageReceipt {
        let (message, receipt) = packet(tag, value);
        self.client.send(self.handle.id(), message);
        receipt
    }
    fn advance(&self, tick: u128) {
        self.client.advance_to(tick).unwrap();
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            drain(retire(task, 2, &self.client.shared));
        }
        assert_eq!(self.client.clock_snapshot().pending, 0);
    }
}
fn timeout(poll: Poll<ReceiveResult>) {
    assert!(matches!(poll, Poll::Ready(ReceiveResult::Timeout)));
}
fn value(poll: Poll<ReceiveResult>, expected: u64) {
    let Poll::Ready(ReceiveResult::Message(message)) = poll else {
        panic!("expected message");
    };
    assert_eq!(message.into_words().read(0), expected);
}

#[test]
fn zero_scans_across_turns_without_arming_and_restores_on_timeout() {
    let case = Case::new();
    let receipts: Vec<_> = (1..=3).map(|i| case.send(i, i * 10)).collect();
    let mut zero = case.timed(3, 0);
    assert!(case.poll(&mut zero, 1).is_pending());
    assert!(!case.handle.snapshot().waiting);
    case.advance(100);
    assert!(case.poll(&mut zero, 1).is_pending());
    value(case.poll(&mut zero, 1), 30);
    let mut absent = case.timed(99, 0);
    timeout(case.poll(&mut absent, 2));
    assert_eq!(
        (
            case.handle.snapshot().queued,
            case.handle.snapshot().scanned_prefix
        ),
        (2, 0)
    );
    for (tag, expected) in [(1, 10), (2, 20)] {
        value(case.poll(&mut case.timed(tag, 0), 2), expected);
    }
    assert_eq!(case.client.clock_snapshot().armed, 0);
    assert!(receipts.iter().all(|r| r.drops() == 1));
}

#[test]
fn finite_deadline_begins_after_initial_scan_and_never_rearms() {
    let case = Case::new();
    case.send(1, 10);
    case.send(2, 20);
    let mut receive = case.timed(7, 5);
    assert!(case.poll(&mut receive, 1).is_pending());
    assert_eq!(case.client.clock_snapshot().armed, 0);
    case.advance(100);
    assert!(case.poll(&mut receive, 1).is_pending());
    assert_eq!(case.handle.snapshot().timer_deadline, Some(105));
    case.advance(104);
    assert_eq!(case.client.shared.clock.service(8), 0);
    case.send(3, 30);
    assert!(case.poll(&mut receive, 1).is_pending());
    assert_eq!(
        (
            case.handle.snapshot().scans,
            case.handle.snapshot().timer_generation
        ),
        (3, Some(1))
    );
    assert_eq!(case.handle.snapshot().timer_deadline, Some(105));
    case.advance(105);
    assert_eq!(case.client.shared.clock.service(8), 1);
    timeout(case.poll(&mut receive, 1));
    assert_eq!(case.client.clock_snapshot().armed, 1);
    assert_eq!(
        (
            case.handle.snapshot().queued,
            case.handle.snapshot().scanned_prefix
        ),
        (3, 0)
    );
}

#[test]
fn queued_match_prevents_arming_even_for_invalid_dynamic_timeout() {
    let case = Case::new();
    for delay in [5, u64::MAX] {
        let receipt = case.send(7, 70);
        value(case.poll(&mut case.timed(7, delay), 1), 70);
        assert_eq!(receipt.drops(), 1);
    }
    assert_eq!(case.client.clock_snapshot().armed, 0);
}

#[test]
fn committed_expiry_precedes_queued_match_at_reentry_in_both_orders() {
    for send_first in [true, false] {
        let case = Case::new();
        let mut receive = case.timed(7, 5);
        assert!(case.poll(&mut receive, 1).is_pending());
        let receipt = if send_first {
            Some(case.send(7, 70))
        } else {
            None
        };
        case.advance(5);
        assert_eq!(case.client.shared.clock.service(8), 1);
        let receipt = receipt.unwrap_or_else(|| case.send(7, 70));
        timeout(case.poll(&mut receive, 1));
        assert_eq!(receipt.drops(), 0);
        value(case.poll(&mut case.timed(7, 0), 1), 70);
        assert_eq!(receipt.drops(), 1);
    }
}

#[test]
fn expiry_interrupts_a_resumed_budget_limited_scan_and_preserves_all() {
    let case = Case::new();
    let mut receive = case.timed(7, 5);
    assert!(case.poll(&mut receive, 1).is_pending());
    for i in 1..=3 {
        case.send(i, i * 10);
    }
    case.send(7, 70);
    assert!(case.poll(&mut receive, 1).is_pending());
    assert!(!case.handle.snapshot().waiting);
    case.advance(5);
    case.client.shared.clock.service(8);
    timeout(case.poll(&mut receive, 1));
    assert_eq!(
        (
            case.handle.snapshot().scans,
            case.handle.snapshot().queued,
            case.handle.snapshot().scanned_prefix
        ),
        (1, 4, 0)
    );
}

#[test]
fn selected_message_cancels_before_service_and_infinity_has_no_timer() {
    let case = Case::new();
    let mut receive = case.timed(7, 5);
    assert!(case.poll(&mut receive, 1).is_pending());
    case.advance(5); // Publication is not acknowledgement of expiry service.
    case.send(7, 70);
    value(case.poll(&mut receive, 1), 70);
    assert_eq!(case.client.shared.clock.service(8), 0);
    assert_eq!(case.client.clock_snapshot().cancelled, 1);
    let task = case.task.as_ref().unwrap();
    let mut forever = Receive::new(task.metrics.clone(), task.budget.clone(), Some(8));
    assert!(receive_poll(&mut forever, &task.budget, 1).is_pending());
    case.advance(1000);
    assert_eq!(case.client.shared.clock.service(8), 0);
    assert!(case.handle.snapshot().waiting);
    assert_eq!(case.client.clock_snapshot().armed, 1);
}

#[test]
fn detached_old_expiry_cannot_timeout_next_receive() {
    let case = Case::new();
    let mut first = case.timed(7, 5);
    assert!(case.poll(&mut first, 1).is_pending());
    case.advance(5);
    let (old_key, old_record) = case.client.shared.clock.take_due().unwrap();
    case.send(7, 70);
    value(case.poll(&mut first, 1), 70);
    let mut second = case.timed(8, 20);
    assert!(case.poll(&mut second, 1).is_pending());
    case.client.shared.clock.expire(old_key, old_record.clone());
    case.client.shared.clock.expire(old_key, old_record);
    assert_eq!(
        (
            case.handle.snapshot().timer_generation,
            case.handle.snapshot().timer_deadline
        ),
        (Some(2), Some(25))
    );
    assert!(!case.handle.snapshot().timer_expired);
    assert!(case.poll(&mut second, 1).is_pending());
    assert_eq!(case.client.clock_snapshot().stale, 2);
}

#[test]
fn expiry_before_park_commit_cannot_lose_early_wake() {
    let mut case = Case::new();
    let mut receive = case.timed(7, 1);
    assert!(case.poll(&mut receive, 1).is_pending());
    case.advance(1);
    case.client.shared.clock.service(8);
    let mut ready = VecDeque::new();
    let mut parked = HashMap::new();
    while let Ok(command) = case.receiver.try_recv() {
        if let Command::Wake(id) = command {
            activate(id, &mut ready, &mut parked);
        }
    }
    pending(case.task.take().unwrap(), &mut ready, &mut parked);
    assert_eq!((ready.len(), parked.len()), (1, 0));
    case.task = ready.pop_front();
    timeout(case.poll(&mut receive, 1));
}

#[test]
fn expiry_after_park_and_duplicate_callback_preserve_single_execution_owner() {
    let mut case = Case::new();
    let mut receive = case.timed(7, 1);
    assert!(case.poll(&mut receive, 1).is_pending());
    let mut ready = VecDeque::new();
    let mut parked = HashMap::new();
    pending(case.task.take().unwrap(), &mut ready, &mut parked);
    case.advance(1);
    let (key, record) = case.client.shared.clock.take_due().unwrap();
    case.client.shared.clock.expire(key, record.clone());
    case.client.shared.clock.expire(key, record);
    while let Ok(command) = case.receiver.try_recv() {
        if let Command::Wake(id) = command {
            activate(id, &mut ready, &mut parked);
        }
    }
    assert_eq!((ready.len(), parked.len()), (1, 0));
    case.task = ready.pop_front();
    timeout(case.poll(&mut receive, 1));
    assert_eq!(
        (
            case.client.clock_snapshot().expired,
            case.client.clock_snapshot().stale
        ),
        (1, 1)
    );
}

#[test]
fn stop_invalidates_detached_timer_before_slot_reuse_and_releases_record() {
    let mut case = Case::new();
    let mut receive = case.timed(7, 1);
    assert!(case.poll(&mut receive, 1).is_pending());
    case.advance(1);
    let (key, record) = case.client.shared.clock.take_due().unwrap();
    let weak = Arc::downgrade(&case.handle.metrics);
    assert_eq!(case.handle.kill(), KillReceipt::Accepted);
    drain(retire(case.task.take().unwrap(), 1, &case.client.shared));
    let replacement = case.client.publish(prepared()).unwrap();
    // Pending clock/wake commands precede this spawn in the test channel.
    let replacement_task = loop {
        if let Command::Spawn(task) = case.receiver.try_recv().unwrap() {
            break task;
        }
    };
    assert_eq!(case.handle.id.slot, replacement.id.slot);
    let before = replacement.snapshot();
    case.client.shared.clock.expire(key, record.clone());
    assert_eq!(replacement.snapshot(), before);
    assert_eq!(
        (
            case.handle.snapshot().terminal,
            case.handle.snapshot().timeouts
        ),
        (2, 0)
    );
    drain(retire(replacement_task, 2, &case.client.shared));
    drop(receive);
    drop(case);
    assert!(weak.upgrade().is_none());
    assert!(matches!(record, timer::Target::Receive(record) if record.upgrade().is_none()));
}

#[test]
fn cancellation_removes_pending_storage_and_shutdown_rejects_clock_updates() {
    let case = Case::new();
    for _ in 0..32 {
        let mut receive = case.timed(7, u64::from(u32::MAX));
        assert!(case.poll(&mut receive, 1).is_pending());
        case.send(7, 70);
        value(case.poll(&mut receive, 1), 70);
        assert_eq!(case.client.clock_snapshot().pending, 0);
    }
    assert_eq!(
        (
            case.client.clock_snapshot().armed,
            case.client.clock_snapshot().cancelled
        ),
        (32, 32)
    );
    case.client.close();
    assert_eq!(case.client.advance_to(1), Err(ClockError::Closed));
    assert_eq!(case.client.clock_snapshot().now, 0);
}

#[test]
fn logical_clock_rejects_backwards_and_deadline_overflow_without_poisoning() {
    let case = Case::new();
    case.advance(u128::MAX - 1);
    assert_eq!(case.client.advance_to(0), Err(ClockError::Backwards));
    let mut exact = case.timed(7, 1);
    assert!(case.poll(&mut exact, 1).is_pending());
    assert_eq!(case.handle.snapshot().timer_deadline, Some(u128::MAX));
    case.advance(u128::MAX);
    case.client.shared.clock.service(8);
    timeout(case.poll(&mut exact, 1));
    let mut overflow = case.timed(7, 1);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| case.poll(&mut overflow, 1)))
            .is_err()
    );
    assert_eq!(case.client.clock_snapshot().pending, 0);
    assert_eq!(case.handle.snapshot().timer_generation, None);
}

#[test]
fn invalid_wait_restores_cursor_and_generation_exhaustion_does_not_wrap() {
    let case = Case::new();
    case.send(1, 10);
    let mut invalid = case.timed(7, u64::MAX);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| case.poll(&mut invalid, 1)))
            .is_err()
    );
    assert_eq!(
        (
            case.handle.snapshot().queued,
            case.handle.snapshot().scanned_prefix
        ),
        (1, 0)
    );
    case.handle.metrics.mailbox.lock().unwrap().next_timer = u64::MAX;
    let mut exhausted = case.timed(7, 1);
    assert!(std::panic::catch_unwind(
        std::panic::AssertUnwindSafe(|| case.poll(&mut exhausted, 1))
    )
    .is_err());
    assert_eq!(case.client.clock_snapshot().armed, 0);
    assert_eq!(
        case.handle.metrics.mailbox.lock().unwrap().next_timer,
        u64::MAX
    );
}

#[test]
fn due_service_is_bounded_and_keeps_remaining_due_work_visible() {
    let cases: Vec<_> = (0..19).map(|_| Case::new()).collect();
    // Share one clock, keeping distinct full process identities.
    let clock = Arc::new(timer::Clock::default());
    for case in &cases {
        let mut inbox = case.handle.metrics.mailbox.lock().unwrap();
        inbox.timer = Some(clock.arm(&case.handle.metrics, 1, 5).unwrap());
        inbox.waiting = true;
    }
    clock.advance_to(5).unwrap();
    assert_eq!(clock.service(8), 8);
    assert!(clock.has_due());
    assert_eq!(clock.service(8), 8);
    assert!(clock.has_due());
    assert_eq!(clock.service(8), 3);
    assert!(!clock.has_due());
    assert_eq!(
        (clock.snapshot().expired, clock.snapshot().service_batches),
        (19, 3)
    );
    assert!(cases
        .iter()
        .all(|case| case.handle.snapshot().timer_expired && !case.handle.snapshot().waiting));
}
