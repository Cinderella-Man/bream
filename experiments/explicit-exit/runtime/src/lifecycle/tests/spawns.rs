use super::monitors::Scene;
use super::*;

const MODES: [InitialRelationship; 3] = [
    InitialRelationship::Link,
    InitialRelationship::Monitor,
    InitialRelationship::LinkMonitor,
];
fn child(s: &Scene, mode: InitialRelationship) -> (ProcessHandle, Option<MonitorRef>, Task) {
    let (handle, reference) = s
        .client
        .publish_initial(prepared(), Some((s.id(0), mode)))
        .unwrap();
    let task = published(&s.receiver);
    (handle, reference, task)
}
fn topology(client: &RuntimeClient, links: usize, monitors: usize) {
    assert_eq!(
        (client.active_links(), client.active_monitors()),
        (links, monitors)
    );
}

#[test]
fn initial_relationships_exist_before_dispatch_and_child_does_not_inherit_trapping() {
    for mode in MODES {
        let s = Scene::new(1);
        s.client.set_trap_exit(s.id(0), true).unwrap();
        let (handle, reference, task) = child(&s, mode);
        topology(
            &s.client,
            usize::from(mode.linked()),
            usize::from(mode.monitored()),
        );
        assert_eq!(reference.is_some(), mode.monitored());
        assert!(!handle.snapshot().trap_exit);
        assert_eq!(handle.snapshot().polls, 0);
        drain(retire(task, 1, &s.client.shared));
        if mode.linked() {
            let Event::Exit(exit) = s.event(0) else {
                panic!("Exit first")
            };
            assert_eq!((exit.from(), exit.reason_code()), (handle.id(), 1));
        }
        if let Some(reference) = reference {
            let Event::Down(down) = s.event(0) else {
                panic!("Down after Exit")
            };
            assert_eq!(
                (down.target(), down.reference(), down.reason_code()),
                (handle.id(), reference, 1)
            );
        }
        assert_eq!(s.handles[0].snapshot().queued, 0);
        topology(&s.client, 0, 0);
    }
}
#[test]
fn immediate_child_failure_reports_committed_reason_without_noproc_gap() {
    for mode in MODES {
        let s = Scene::new(1);
        s.client.set_trap_exit(s.id(0), true).unwrap();
        let (handle, reference, task) = child(&s, mode);
        drain(retire(task, 3, &s.client.shared));
        assert_eq!(handle.snapshot().exit_reason, 3);
        if mode.linked() {
            let Event::Exit(exit) = s.event(0) else {
                panic!("Exit missing")
            };
            assert_eq!((exit.from(), exit.reason_code()), (handle.id(), 3));
        }
        if let Some(reference) = reference {
            let down = s.down(0, reference);
            assert_eq!((down.target(), down.reason_code()), (handle.id(), 3));
        }
    }
}
#[test]
fn normal_parent_stop_leaves_default_child_alive_and_cancels_its_monitor() {
    for mode in MODES {
        let mut s = Scene::new(1);
        let (handle, _, task) = child(&s, mode);
        drain(s.retire(0, 1));
        assert_eq!(
            (
                handle.snapshot().terminal,
                handle.snapshot().pending_exit,
                handle.snapshot().queued
            ),
            (0, 0, 0)
        );
        topology(&s.client, 0, 0);
        drain(retire(task, 1, &s.client.shared));
    }
}
#[test]
fn abnormal_parent_stop_propagates_only_over_initial_link() {
    for mode in MODES {
        let mut s = Scene::new(1);
        s.client.set_trap_exit(s.id(0), true).unwrap();
        let (handle, _, task) = child(&s, mode);
        drain(s.retire(0, 2));
        assert_eq!(
            handle.snapshot().pending_exit,
            if mode.linked() { 2 } else { 0 }
        );
        assert_eq!(handle.snapshot().queued, 0);
        drain(retire(task, 1, &s.client.shared));
        assert_eq!(
            handle.snapshot().exit_reason,
            if mode.linked() { 2 } else { 1 }
        );
        topology(&s.client, 0, 0);
    }
}
#[test]
fn pending_parent_kill_can_publish_before_actual_stop() {
    let mut s = Scene::new(1);
    assert_eq!(s.handles[0].kill(), KillReceipt::Accepted);
    assert!(matches!(s.receiver.try_recv(), Ok(Command::Wake(id)) if id == s.id(0)));
    let (handle, _, task) = child(&s, InitialRelationship::LinkMonitor);
    topology(&s.client, 1, 1);
    drain(s.retire(0, 1));
    assert_eq!(handle.snapshot().pending_exit, 2);
    drain(retire(task, 1, &s.client.shared));
    assert_eq!(handle.snapshot().exit_reason, 2);
}
#[test]
fn stopped_or_reused_origin_cannot_publish_a_child() {
    let mut s = Scene::new(1);
    let old = s.id(0);
    drain(s.retire(0, 1));
    let replacement = s.client.publish(prepared()).unwrap();
    let replacement_task = published(&s.receiver);
    assert_eq!(replacement.id().slot, old.slot);
    assert_ne!(replacement.id().generation, old.generation);
    for mode in MODES {
        rejection(
            s.client
                .publish_initial(prepared(), Some((old, mode)))
                .err()
                .unwrap(),
            SpawnErrorKind::OriginNotLive,
        );
    }
    assert_eq!(replacement.snapshot().polls, 0);
    assert_eq!(replacement.snapshot().queued, 0);
    topology(&s.client, 0, 0);
    assert_eq!(s.client.live_count(), 1);
    drain(retire(replacement_task, 1, &s.client.shared));
}
#[test]
fn foreign_or_invalid_origin_never_installs_relationships() {
    let s = Scene::new(1);
    for origin in [
        ProcessId {
            runtime: s.id(0).runtime + 1,
            ..s.id(0)
        },
        ProcessId {
            slot: usize::MAX,
            ..s.id(0)
        },
    ] {
        rejection(
            s.client
                .publish_initial(prepared(), Some((origin, InitialRelationship::LinkMonitor)))
                .err()
                .unwrap(),
            SpawnErrorKind::OriginNotLive,
        );
    }
    topology(&s.client, 0, 0);
    assert_eq!(s.client.live_count(), 1);
    assert_eq!(s.handles[0].snapshot().queued, 0);
}
#[test]
fn capacity_rejection_keeps_origin_and_drops_unpublished_input_once() {
    let (client, receiver) = fixture();
    let parent = client.publish(prepared()).unwrap();
    for mode in MODES {
        rejection(
            client
                .publish_initial(prepared(), Some((parent.id(), mode)))
                .err()
                .unwrap(),
            SpawnErrorKind::Capacity,
        );
    }
    topology(&client, 0, 0);
    assert_eq!(client.live_count(), 1);
    drain(retire(published(&receiver), 1, &client.shared));
    assert!(receiver.try_recv().is_err());
}
#[test]
fn close_before_child_publication_rejects_without_notifications() {
    let s = Scene::new(1);
    let candidate = prepared();
    s.client.close();
    rejection(
        s.client
            .publish_initial(candidate, Some((s.id(0), InitialRelationship::LinkMonitor)))
            .err()
            .unwrap(),
        SpawnErrorKind::Closed,
    );
    topology(&s.client, 0, 0);
    assert_eq!(s.handles[0].snapshot().queued, 0);
    assert!(matches!(s.receiver.try_recv(), Ok(Command::Shutdown)));
    assert!(s.receiver.try_recv().is_err());
}
#[test]
fn initial_spawn_command_precedes_shutdown_marker() {
    let s = Scene::new(1);
    let (handle, reference) = s
        .client
        .publish_initial(
            prepared(),
            Some((s.id(0), InitialRelationship::LinkMonitor)),
        )
        .unwrap();
    s.client.close();
    let task = published(&s.receiver);
    assert!(matches!(s.receiver.try_recv(), Ok(Command::Shutdown)));
    assert!(reference.is_some());
    topology(&s.client, 1, 1);
    drain(retire(task, 2, &s.client.shared));
    assert_eq!((handle.snapshot().polls, handle.snapshot().drops), (0, 1));
}
#[test]
fn disconnected_dispatch_rolls_back_both_relationships_without_exit_or_down() {
    for mode in MODES {
        let (client, receiver) = fixture();
        *client.shared.registry.lock().unwrap() = Registry::new(2);
        let parent = client.publish(prepared()).unwrap();
        let parent_task = published(&receiver);
        drop(receiver);
        rejection(
            client
                .publish_initial(prepared(), Some((parent.id(), mode)))
                .err()
                .unwrap(),
            SpawnErrorKind::Disconnected,
        );
        topology(&client, 0, 0);
        assert_eq!(client.live_count(), 1);
        assert_eq!(
            (parent.snapshot().queued, parent.snapshot().pending_exit),
            (0, 0)
        );
        let registry = client.shared.registry.lock().unwrap();
        assert!(!registry.accepting);
        assert_eq!(registry.slots[1].generation, 2);
        assert!(registry.slots[1].record.is_none());
        drop(registry);
        drain(retire(parent_task, 1, &client.shared));
    }
}
#[test]
fn exhausted_monitor_identity_rolls_back_child_and_leaves_link_spawn_available() {
    let s = Scene::new(1);
    s.client.shared.monitors.exhaust_references();
    for mode in [
        InitialRelationship::Monitor,
        InitialRelationship::LinkMonitor,
    ] {
        rejection(
            s.client
                .publish_initial(prepared(), Some((s.id(0), mode)))
                .err()
                .unwrap(),
            SpawnErrorKind::MonitorExhausted,
        );
        topology(&s.client, 0, 0);
        assert_eq!(s.client.live_count(), 1);
        assert!(s.receiver.try_recv().is_err());
    }
    let (_, reference, task) = child(&s, InitialRelationship::Link);
    assert!(reference.is_none());
    drain(retire(task, 1, &s.client.shared));
}
#[test]
fn repeated_monitored_children_return_distinct_exact_references() {
    let s = Scene::new(1);
    let (first, a, t1) = child(&s, InitialRelationship::Monitor);
    drain(retire(t1, 1, &s.client.shared));
    let (second, b, t2) = child(&s, InitialRelationship::Monitor);
    assert_eq!(first.id().slot, second.id().slot);
    assert_ne!(first.id(), second.id());
    assert_ne!(a, b);
    drain(retire(t2, 3, &s.client.shared));
    let one = s.down(0, a.unwrap());
    let two = s.down(0, b.unwrap());
    assert_eq!((one.target(), one.reason_code()), (first.id(), 1));
    assert_eq!((two.target(), two.reason_code()), (second.id(), 3));
}
fn drop_context(context: ProcessContext) -> Frame {
    Box::pin(async move {
        drop(context);
    })
}
#[test]
fn transferred_input_rejection_drops_at_original_receipt_once() {
    let s = Scene::new(1);
    let allocation = Arc::new(Allocation::default());
    allocation.live_bytes.store(64, Ordering::Relaxed);
    let words = OwnedWords {
        words: vec![7; 8],
        allocation: allocation.clone(),
    };
    let candidate = Prepared::from_owned(
        Program { make: drop_context },
        words,
        3,
        4,
        &s.client.shared,
        s.id(0),
    );
    s.client.close();
    let error = s
        .client
        .publish_initial(candidate, Some((s.id(0), InitialRelationship::LinkMonitor)))
        .err()
        .unwrap();
    assert_eq!(error.kind, SpawnErrorKind::Closed);
    let snapshot = error.cleanup.unwrap();
    assert_eq!(
        (
            snapshot.polls,
            snapshot.live_bytes,
            snapshot.drops,
            snapshot.terminal
        ),
        (0, 0, 0, 4)
    );
    assert_eq!(
        (
            allocation.live_bytes.load(Ordering::Acquire),
            allocation.freed_bytes.load(Ordering::Acquire),
            allocation.drops.load(Ordering::Acquire)
        ),
        (0, 64, 1)
    );
    topology(&s.client, 0, 0);
}
#[test]
fn actual_first_child_poll_observes_initial_topology() {
    let mut s = Scene::new(1);
    s.client.set_trap_exit(s.id(0), true).unwrap();
    let (tx, rx) = mpsc::channel();
    let mut candidate = prepared();
    let original = candidate.frame;
    let client = s.client.clone();
    candidate.frame = Box::pin(async move {
        tx.send((client.active_links(), client.active_monitors()))
            .unwrap();
        original.await;
    });
    let (_, empty) = mpsc::channel();
    let receiver = std::mem::replace(&mut s.receiver, empty);
    let shared = s.client.shared.clone();
    let worker = std::thread::spawn(move || crate::worker(receiver, 1, shared, true));
    let (handle, reference) = s
        .client
        .publish_initial(candidate, Some((s.id(0), InitialRelationship::LinkMonitor)))
        .unwrap();
    let observed = rx.recv_timeout(std::time::Duration::from_secs(10));
    s.client.close();
    worker.join().unwrap();
    assert_eq!(observed.unwrap(), (1, 1));
    assert_eq!(
        (
            handle.snapshot().terminal,
            handle.snapshot().polls,
            handle.snapshot().drops
        ),
        (1, 1, 1)
    );
    let Event::Exit(exit) = s.event(0) else {
        panic!("Exit first")
    };
    let Event::Down(down) = s.event(0) else {
        panic!("Down second")
    };
    assert_eq!(
        (exit.reason_code(), down.reason_code(), down.reference()),
        (1, 1, reference.unwrap())
    );
}
