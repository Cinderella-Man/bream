use super::*;

fn fixture() -> (RuntimeClient, mpsc::Receiver<Command>) {
    let (sender, receiver) = mpsc::channel();
    let shared = Arc::new(Shared {
        epoch: Instant::now(),
        namespace: next_namespace(&NEXT_RUNTIME).unwrap(),
        registry: Mutex::new(Registry::new(1)),
        sender,
        rejected: AtomicUsize::new(0),
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
        .live_bytes
        .store(words.capacity() as u64 * 8, Ordering::Relaxed);
    let owner = OwnedWords {
        words,
        metrics: metrics.clone(),
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
    assert!(s.stop_ns > 0 && s.cleanup_ns >= s.stop_ns);
}

#[test]
fn slot_reuse_between_stop_and_old_cleanup() {
    let (client, receiver) = fixture();
    let old = client.publish(prepared()).ok().unwrap();
    let task = published(&receiver);
    assert_eq!(old.kill(), KillReceipt::Accepted);
    stop(&task, 1, &client.shared);
    let stopped = old.snapshot();
    assert_eq!(
        (stopped.terminal, stopped.cleanup_ns, stopped.live_bytes),
        (2, 0, 64)
    );
    assert!(matches!(client.lookup(old.id()), Err(LookupError::NotLive)));
    let replacement = client.publish(prepared()).ok().unwrap();
    assert_eq!(old.id.slot, replacement.id.slot);
    assert_eq!(old.id.generation + 1, replacement.id.generation);
    let before = replacement.snapshot();
    assert_eq!(old.kill(), KillReceipt::TooLate);
    assert_eq!(client.kill(old.id()), Err(LookupError::NotLive));
    cleanup(task, client.shared.epoch);
    assert_eq!(replacement.snapshot(), before);
    assert_eq!(
        client.lookup(replacement.id()).ok().unwrap().id(),
        replacement.id()
    );
    assert_eq!(old.snapshot().drops, 1);
    retire(published(&receiver), 2, &client.shared);
    assert_eq!(client.live_count(), 0);
}

#[test]
fn accepted_kill_wins_over_uncommitted_ready_or_panic() {
    for reason in [1, 3] {
        let (client, receiver) = fixture();
        let handle = client.publish(prepared()).ok().unwrap();
        assert_eq!(handle.kill(), KillReceipt::Accepted);
        assert_eq!(handle.kill(), KillReceipt::AlreadyRequested);
        retire(published(&receiver), reason, &client.shared);
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
    retire(task, 1, &client.shared);
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
        retire(published(&receiver), reason, &client.shared);
        let finished = handle.snapshot();
        assert_eq!((finished.terminal, finished.kill_ns), (reason, 0));
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
    retire(task, 2, &client.shared);
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
    retire(published(&receiver), 2, &client.shared);
}

#[test]
fn generation_exhaustion_retires_slot_instead_of_wrapping() {
    let (client, receiver) = fixture();
    client.shared.registry.lock().unwrap().slots[0].generation = u64::MAX;
    let owner = client.publish(prepared()).ok().unwrap();
    assert_eq!(owner.id.generation, u64::MAX);
    retire(published(&receiver), 2, &client.shared);
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
    retire(published(&receiver), 1, &client.shared);
}
