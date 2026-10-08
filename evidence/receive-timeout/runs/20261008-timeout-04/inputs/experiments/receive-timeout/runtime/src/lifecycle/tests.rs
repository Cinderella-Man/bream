use super::*;

fn fixture() -> (RuntimeClient, mpsc::Receiver<Command>) {
    let (sender, receiver) = mpsc::channel();
    let shared = Arc::new(Shared {
        namespace: next_namespace(&NEXT_RUNTIME).unwrap(),
        registry: Mutex::new(Registry::new(1)),
        sender,
        rejected: AtomicUsize::new(0),
        clock: Arc::new(timer::Clock::default()),
    });
    (RuntimeClient { shared }, receiver)
}
fn prepared() -> Prepared {
    let metrics = Arc::new(Metrics::default());
    let budget = Arc::new(Budget {
        remaining: AtomicU32::new(0),
    });
    let words = vec![7_u64; 8];
    metrics
        .allocation
        .live_bytes
        .store(words.capacity() as u64 * 8, Ordering::Relaxed);
    let owner = OwnedWords {
        words,
        allocation: metrics.allocation.clone(),
    };
    // Internal protocol fixture, never exposed through safe Program admission.
    let frame = Box::pin(async move {
        drop(owner);
    });
    Prepared {
        frame,
        metrics,
        budget,
    }
}
fn published(receiver: &mpsc::Receiver<Command>) -> Task {
    match receiver.try_recv().unwrap() {
        Command::Spawn(task) => task,
        _ => panic!("expected spawn"),
    }
}
fn rejection(error: SpawnError, kind: SpawnErrorKind) {
    assert_eq!(error.kind, kind);
    let s = error.cleanup.unwrap();
    assert_eq!(
        (s.polls, s.live_bytes, s.freed_bytes, s.drops, s.terminal),
        (0, 0, 64, 1, 4)
    );
    assert!(s.stop_event > 0 && s.cleanup_event >= s.stop_event);
}

#[test]
fn slot_reuse_between_stop_and_old_cleanup() {
    let (client, receiver) = fixture();
    let old = client.publish(prepared()).ok().unwrap();
    let task = published(&receiver);
    assert_eq!(old.kill(), KillReceipt::Accepted);
    let pending = stop(&task, 1, &client.shared);
    let stopped = old.snapshot();
    assert_eq!(
        (stopped.terminal, stopped.cleanup_event, stopped.live_bytes),
        (2, 0, 64)
    );
    assert!(matches!(client.lookup(old.id()), Err(LookupError::NotLive)));
    let replacement = client.publish(prepared()).ok().unwrap();
    assert_eq!(old.id.slot, replacement.id.slot);
    assert_eq!(old.id.generation + 1, replacement.id.generation);
    let before = replacement.snapshot();
    assert_eq!(old.kill(), KillReceipt::TooLate);
    assert_eq!(client.kill(old.id()), Err(LookupError::NotLive));
    assert!(cleanup(task, pending).step());
    assert_eq!(replacement.snapshot(), before);
    assert_eq!(
        client.lookup(replacement.id()).ok().unwrap().id(),
        replacement.id()
    );
    assert_eq!(old.snapshot().drops, 1);
    assert!(retire(published(&receiver), 2, &client.shared).step());
    assert_eq!(client.live_count(), 0);
}

#[test]
fn accepted_kill_wins_over_uncommitted_ready_or_panic() {
    for reason in [1, 3] {
        let (client, receiver) = fixture();
        let handle = client.publish(prepared()).ok().unwrap();
        assert_eq!(handle.kill(), KillReceipt::Accepted);
        assert_eq!(handle.kill(), KillReceipt::AlreadyRequested);
        assert!(retire(published(&receiver), reason, &client.shared).step());
        let finished = handle.snapshot();
        assert_eq!(finished.terminal, 2);
        assert_eq!(handle.kill(), KillReceipt::TooLate);
        assert_eq!(handle.snapshot(), finished);
    }
}

#[test]
fn accepted_kill_after_actual_ready_poll_wins_terminal_claim() {
    let (client, receiver) = fixture();
    let handle = client.publish(prepared()).ok().unwrap();
    let mut task = published(&receiver);
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    assert!(task.frame.as_mut().poll(&mut context).is_ready());
    assert_eq!(handle.snapshot().drops, 1);
    assert_eq!(handle.kill(), KillReceipt::Accepted);
    assert!(retire(task, 1, &client.shared).step());
    assert_eq!(
        (handle.snapshot().terminal, handle.snapshot().drops),
        (2, 1)
    );
}

#[test]
fn committed_completion_or_failure_cannot_be_rewritten() {
    for reason in [1, 3] {
        let (client, receiver) = fixture();
        let handle = client.publish(prepared()).ok().unwrap();
        assert!(retire(published(&receiver), reason, &client.shared).step());
        let finished = handle.snapshot();
        assert_eq!((finished.terminal, finished.kill_event), (reason, 0));
        assert_eq!(handle.kill(), KillReceipt::TooLate);
        assert_eq!(handle.snapshot(), finished);
        assert!(matches!(
            client.lookup(handle.id()),
            Err(LookupError::NotLive)
        ));
    }
}

#[test]
fn prepared_before_shutdown_but_published_after_is_rejected() {
    let (client, receiver) = fixture();
    let pending = prepared();
    client.close();
    rejection(
        client.publish(pending).err().unwrap(),
        SpawnErrorKind::Closed,
    );
    assert!(matches!(receiver.try_recv(), Ok(Command::Shutdown)));
    assert!(receiver.try_recv().is_err());
    assert_eq!(client.live_count(), 0);
}

#[test]
fn accepted_spawn_precedes_shutdown_marker() {
    let (client, receiver) = fixture();
    let handle = client.publish(prepared()).ok().unwrap();
    client.close();
    let task = published(&receiver);
    assert!(matches!(receiver.try_recv(), Ok(Command::Shutdown)));
    assert!(retire(task, 2, &client.shared).step());
    assert_eq!((handle.snapshot().polls, handle.snapshot().drops), (0, 1));
    rejection(
        client.publish(prepared()).err().unwrap(),
        SpawnErrorKind::Closed,
    );
}

#[test]
fn disconnected_enqueue_rolls_back_exact_record_and_input() {
    let (client, receiver) = fixture();
    drop(receiver);
    rejection(
        client.publish(prepared()).err().unwrap(),
        SpawnErrorKind::Disconnected,
    );
    let registry = client.shared.registry.lock().unwrap();
    assert_eq!(
        (
            registry.live,
            registry.free.len(),
            registry.slots[0].generation
        ),
        (0, 1, 2)
    );
    assert!(registry.slots[0].record.is_none() && !registry.accepting);
}

#[test]
fn full_registry_rejection_does_not_replace_owner() {
    let (client, receiver) = fixture();
    let owner = client.publish(prepared()).ok().unwrap();
    let before = owner.snapshot();
    rejection(
        client.publish(prepared()).err().unwrap(),
        SpawnErrorKind::Capacity,
    );
    assert_eq!(owner.snapshot(), before);
    assert_eq!(client.lookup(owner.id()).ok().unwrap().id(), owner.id());
    assert!(retire(published(&receiver), 2, &client.shared).step());
}

#[test]
fn generation_exhaustion_retires_slot_instead_of_wrapping() {
    let (client, receiver) = fixture();
    client.shared.registry.lock().unwrap().slots[0].generation = u64::MAX;
    let owner = client.publish(prepared()).ok().unwrap();
    assert_eq!(owner.id.generation, u64::MAX);
    assert!(retire(published(&receiver), 2, &client.shared).step());
    assert_eq!((client.live_count(), client.exhausted_slots()), (0, 1));
    rejection(
        client.publish(prepared()).err().unwrap(),
        SpawnErrorKind::Capacity,
    );
    assert!(matches!(
        client.lookup(owner.id()),
        Err(LookupError::NotLive)
    ));
}

#[test]
fn namespace_exhaustion_never_reuses_a_number() {
    let counter = AtomicU64::new(u64::MAX - 1);
    assert_eq!(next_namespace(&counter), Some(u64::MAX - 1));
    assert_eq!(next_namespace(&counter), None);
    assert_eq!(next_namespace(&counter), None);
    assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
}

#[test]
fn lookup_validates_namespace_slot_and_generation() {
    let (client, receiver) = fixture();
    let owner = client.publish(prepared()).ok().unwrap();
    let id = owner.id();
    assert!(matches!(
        client.lookup(ProcessId {
            runtime: id.runtime + 1,
            ..id
        }),
        Err(LookupError::WrongRuntime)
    ));
    assert!(matches!(
        client.lookup(ProcessId {
            slot: usize::MAX,
            ..id
        }),
        Err(LookupError::NotLive)
    ));
    assert!(matches!(
        client.lookup(ProcessId {
            generation: id.generation + 1,
            ..id
        }),
        Err(LookupError::NotLive)
    ));
    assert!(retire(published(&receiver), 1, &client.shared).step());
}

fn packet(tag: u64, value: u64) -> (Envelope, MessageReceipt) {
    Envelope::new(tag, vec![value]).unwrap()
}
fn receive_poll(receive: &mut Receive, budget: &Budget, allowance: u32) -> Poll<Envelope> {
    budget.remaining.store(allowance, Ordering::Relaxed);
    let waker = Waker::from(Arc::new(NoopWake));
    Pin::new(receive).poll(&mut Context::from_waker(&waker))
}
fn drain(mut cleanup: Cleanup) {
    while !cleanup.step() {}
}

#[test]
fn selective_scan_spends_budget_and_preserves_prefix_without_rescan() {
    let metrics = Arc::new(Metrics::default());
    let budget = Arc::new(Budget {
        remaining: AtomicU32::new(0),
    });
    let mut receipts = Vec::new();
    for (tag, value) in [(1, 10), (2, 20), (1, 30), (3, 40)] {
        let (message, receipt) = packet(tag, value);
        mailbox::deliver(&metrics, message);
        receipts.push(receipt);
    }
    let mut receive = Receive::new(metrics.clone(), budget.clone(), Some(3));
    assert!(receive_poll(&mut receive, &budget, 2).is_pending());
    assert_eq!(metrics.scans.load(Ordering::Relaxed), 2);
    assert_eq!(snapshot(&metrics).scanned_prefix, 2);
    assert!(!snapshot(&metrics).waiting);
    let Poll::Ready(selected) = receive_poll(&mut receive, &budget, 2) else {
        panic!("match")
    };
    assert_eq!(selected.into_words().read(0), 40);
    assert_eq!(metrics.scans.load(Ordering::Relaxed), 4);
    assert_eq!(
        (snapshot(&metrics).queued, snapshot(&metrics).scanned_prefix),
        (3, 0)
    );
    for value in [10, 20, 30] {
        let mut any = Receive::new(metrics.clone(), budget.clone(), None);
        let Poll::Ready(message) = receive_poll(&mut any, &budget, 1) else {
            panic!("retained message")
        };
        assert_eq!(message.into_words().read(0), value);
    }
    assert!(receipts
        .iter()
        .all(|r| r.drops() == 1 && r.freed_bytes() == 8));
}

#[test]
fn unmatched_arrival_does_not_rescan_saved_messages() {
    let metrics = Arc::new(Metrics::default());
    let budget = Arc::new(Budget {
        remaining: AtomicU32::new(0),
    });
    mailbox::deliver(&metrics, packet(1, 10).0);
    let mut receive = Receive::new(metrics.clone(), budget.clone(), Some(7));
    assert!(receive_poll(&mut receive, &budget, 8).is_pending());
    assert!(snapshot(&metrics).waiting);
    mailbox::deliver(&metrics, packet(2, 20).0);
    assert!(receive_poll(&mut receive, &budget, 8).is_pending());
    assert_eq!(snapshot(&metrics).scans, 2);
    mailbox::deliver(&metrics, packet(7, 70).0);
    assert!(receive_poll(&mut receive, &budget, 8).is_ready());
    assert_eq!(snapshot(&metrics).scans, 3);
    drop(metrics.mailbox.lock().unwrap().close());
}

#[test]
fn send_before_park_commit_forces_requeue_even_if_wake_was_consumed() {
    let (client, receiver) = fixture();
    let handle = client.publish(prepared()).ok().unwrap();
    let task = published(&receiver);
    task.metrics
        .runtime
        .set(Arc::downgrade(&client.shared))
        .unwrap();
    task.metrics.mailbox.lock().unwrap().waiting = true;
    client.send(handle.id(), packet(7, 70).0);
    let Command::Wake(woken) = receiver.try_recv().unwrap() else {
        panic!("send wake")
    };
    assert_eq!(woken, handle.id());
    let mut ready = VecDeque::new();
    let mut parked = HashMap::new();
    activate(handle.id(), &mut ready, &mut parked);
    pending(task, &mut ready, &mut parked);
    assert!(parked.is_empty() && ready.len() == 1);
    drain(retire(ready.pop_front().unwrap(), 2, &client.shared));
}

#[test]
fn send_after_park_and_repeated_wakes_keep_single_execution_owner() {
    let (client, receiver) = fixture();
    let handle = client.publish(prepared()).ok().unwrap();
    let task = published(&receiver);
    task.metrics
        .runtime
        .set(Arc::downgrade(&client.shared))
        .unwrap();
    task.metrics.mailbox.lock().unwrap().waiting = true;
    let mut ready = VecDeque::new();
    let mut parked = HashMap::new();
    pending(task, &mut ready, &mut parked);
    assert_eq!(parked.len(), 1);
    client.send(handle.id(), packet(7, 70).0);
    let Command::Wake(woken) = receiver.try_recv().unwrap() else {
        panic!("send wake")
    };
    assert_eq!(woken, handle.id());
    activate(handle.id(), &mut ready, &mut parked);
    activate(handle.id(), &mut ready, &mut parked);
    assert!(parked.is_empty() && ready.len() == 1);
    assert_eq!(handle.snapshot().wakes, 1);
    drain(retire(ready.pop_front().unwrap(), 2, &client.shared));
}

#[test]
fn kill_of_parked_process_schedules_only_terminal_work() {
    let (client, receiver) = fixture();
    let handle = client.publish(prepared()).ok().unwrap();
    let task = published(&receiver);
    task.metrics
        .runtime
        .set(Arc::downgrade(&client.shared))
        .unwrap();
    task.metrics.mailbox.lock().unwrap().waiting = true;
    let mut ready = VecDeque::new();
    let mut parked = HashMap::new();
    pending(task, &mut ready, &mut parked);
    assert_eq!(handle.kill(), KillReceipt::Accepted);
    let Command::Wake(id) = receiver.try_recv().unwrap() else {
        panic!("kill wake")
    };
    activate(id, &mut ready, &mut parked);
    let task = ready.pop_front().unwrap();
    assert_ne!(task.metrics.kill_event.load(Ordering::Acquire), 0);
    drain(retire(task, 2, &client.shared));
    assert_eq!(
        (handle.snapshot().terminal, handle.snapshot().polls),
        (2, 0)
    );
}

#[test]
fn lookup_before_close_cannot_publish_into_retired_mailbox() {
    let (client, receiver) = fixture();
    let old = client.publish(prepared()).ok().unwrap();
    let retained = client.lookup(old.id()).unwrap();
    drain(retire(published(&receiver), 2, &client.shared));
    let (message, receipt) = packet(7, 70);
    mailbox::deliver(&retained.metrics, message);
    assert_eq!(receipt.drops(), 1);
    assert_eq!(retained.snapshot().queued, 0);
}

#[test]
fn stale_wake_after_reuse_cannot_unpark_replacement() {
    let (client, receiver) = fixture();
    let old = client.publish(prepared()).ok().unwrap();
    drain(retire(published(&receiver), 2, &client.shared));
    let new = client.publish(prepared()).ok().unwrap();
    let task = published(&receiver);
    task.metrics.mailbox.lock().unwrap().waiting = true;
    let mut ready = VecDeque::new();
    let mut parked = HashMap::new();
    pending(task, &mut ready, &mut parked);
    activate(old.id(), &mut ready, &mut parked);
    assert!(ready.is_empty() && parked.contains_key(&new.id()));
    let (message, receipt) = packet(7, 70);
    client.send(old.id(), message);
    assert_eq!(receipt.drops(), 1);
    assert_eq!(new.snapshot().queued, 0);
    drain(retire(parked.remove(&new.id()).unwrap(), 2, &client.shared));
}

#[test]
fn stop_detaches_saved_and_incoming_then_cleanup_is_batched() {
    let (client, receiver) = fixture();
    let handle = client.publish(prepared()).ok().unwrap();
    let task = published(&receiver);
    let mut receipts = Vec::new();
    for _ in 0..19 {
        let (message, receipt) = packet(1, 10);
        client.send(handle.id(), message);
        receipts.push(receipt);
    }
    let mut receive = Receive::new(task.metrics.clone(), task.budget.clone(), Some(7));
    assert!(receive_poll(&mut receive, &task.budget, 3).is_pending());
    assert_eq!(handle.snapshot().scanned_prefix, 3);
    let mut cleanup = retire(task, 2, &client.shared);
    assert_eq!(
        (handle.snapshot().queued, handle.snapshot().cleanup_event),
        (0, 0)
    );
    assert!(!cleanup.step());
    assert_eq!(receipts.iter().filter(|r| r.drops() == 1).count(), 8);
    assert!(!cleanup.step());
    assert_eq!(receipts.iter().filter(|r| r.drops() == 1).count(), 16);
    assert!(cleanup.step());
    assert!(receipts.iter().all(|r| r.drops() == 1));
    assert_eq!(
        (
            handle.snapshot().cleanup_batches,
            handle.snapshot().cleanup_messages
        ),
        (3, 19)
    );
}

#[test]
fn allocation_receipt_cannot_retain_a_process_record() {
    let (client, receiver) = fixture();
    let handle = client.publish(prepared()).ok().unwrap();
    let weak_record = Arc::downgrade(&handle.metrics);
    let allocation = handle.metrics.allocation.clone();
    drain(retire(published(&receiver), 2, &client.shared));
    drop(handle);
    assert!(weak_record.upgrade().is_none());
    assert_eq!(allocation.drops.load(Ordering::Acquire), 1);
}

#[test]
fn rejected_scan_transfers_existing_nodes_without_rebuilding_prefix() {
    let metrics = Arc::new(Metrics::default());
    let budget = Arc::new(Budget {
        remaining: AtomicU32::new(0),
    });
    for tag in [1, 2, 3] {
        mailbox::deliver(&metrics, packet(tag, tag).0);
    }
    let before: Vec<_> = metrics
        .mailbox
        .lock()
        .unwrap()
        .incoming
        .iter()
        .map(std::ptr::from_ref)
        .collect();
    let mut receive = Receive::new(metrics.clone(), budget.clone(), Some(3));
    assert!(receive_poll(&mut receive, &budget, 2).is_pending());
    let saved: Vec<_> = metrics
        .mailbox
        .lock()
        .unwrap()
        .saved
        .iter()
        .map(std::ptr::from_ref)
        .collect();
    assert_eq!(saved, before[..2]);
    assert!(receive_poll(&mut receive, &budget, 1).is_ready());
    let restored: Vec<_> = metrics
        .mailbox
        .lock()
        .unwrap()
        .incoming
        .iter()
        .map(std::ptr::from_ref)
        .collect();
    assert_eq!(restored, before[..2]);
    drop(metrics.mailbox.lock().unwrap().close());
}

mod timeouts;
