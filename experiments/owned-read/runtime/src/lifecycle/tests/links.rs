use super::monitors::{poll, Scene};
use super::*;

fn connect(s: &Scene, a: usize, b: usize) {
    s.client.link(s.id(a), s.id(b)).unwrap();
}
fn trapping(s: &Scene, i: usize, value: bool) -> bool {
    s.client.set_trap_exit(s.id(i), value).unwrap()
}
fn take_exit(s: &Scene, i: usize, from: usize) -> Exit {
    let task = s.tasks[i].as_ref().unwrap();
    let mut receive =
        mailbox::ExitReceive::new(task.metrics.clone(), task.budget.clone(), s.id(from));
    let Poll::Ready(exit) = poll(&mut receive, &task.budget, 100) else {
        panic!("Exit absent")
    };
    assert_eq!(exit.from(), s.id(from));
    exit
}

#[test]
fn self_and_repeated_links_are_idempotent_and_weak() {
    let s = Scene::new(2);
    let holds = Arc::strong_count(&s.handles[0].metrics);
    for _ in 0..64 {
        connect(&s, 0, 0);
    }
    assert_eq!(s.client.active_links(), 0);
    for _ in 0..64 {
        connect(&s, 0, 1);
        connect(&s, 1, 0);
    }
    assert_eq!(s.client.active_links(), 1);
    assert_eq!(Arc::strong_count(&s.handles[0].metrics), holds);
    assert!(matches!(
        s.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    s.client.unlink(s.id(1), s.id(0));
    s.client.unlink(s.id(0), s.id(1));
    assert_eq!(s.client.active_links(), 0);
}
#[test]
fn normal_untrapped_exit_removes_topology_without_application_effect() {
    let mut s = Scene::new(2);
    connect(&s, 0, 1);
    drain(s.retire(1, 1));
    assert_eq!(s.client.active_links(), 0);
    let snapshot = s.handles[0].snapshot();
    assert_eq!(
        (snapshot.queued, snapshot.pending_exit, snapshot.terminal),
        (0, 0, 0)
    );
    assert!(s.client.is_live(s.id(0)));
}
#[test]
fn trapped_normal_killed_and_failure_preserve_source_and_reason() {
    for reason in [1, 2, 3] {
        let mut s = Scene::new(2);
        assert!(!trapping(&s, 0, true));
        assert!(trapping(&s, 0, true));
        connect(&s, 0, 1);
        let mut cleanup = s.retire(1, reason);
        assert!(s.handles[1].snapshot().stop_event > 0);
        assert!(!s.client.is_live(s.id(1)));
        assert!(cleanup.step());
        assert_eq!(take_exit(&s, 0, 1).reason_code(), u64::from(reason));
        assert_eq!(s.client.active_links(), 0);
    }
}
#[test]
fn pending_fatal_is_distinct_from_direct_kill_and_keeps_reason_through_monitor() {
    let mut s = Scene::new(3);
    connect(&s, 0, 1);
    let reference = s.monitor(2, 0);
    drain(s.retire(1, 2));
    let before = s.handles[0].snapshot();
    assert_eq!(
        (before.pending_exit, before.kill_event, before.terminal),
        (2, 0, 0)
    );
    assert_eq!(s.handles[0].kill(), KillReceipt::TooLate);
    drain(s.retire(0, 3));
    let done = s.handles[0].snapshot();
    assert_eq!((done.terminal, done.exit_reason), (3, 2));
    assert_eq!(s.down(2, reference).reason_code(), 2);
}
#[test]
fn trap_choice_is_at_conversion_and_does_not_retract_queued_exit() {
    for enabled in [false, true] {
        let mut s = Scene::new(2);
        trapping(&s, 0, !enabled);
        connect(&s, 0, 1);
        let cleanup = s.retire(1, 3);
        trapping(&s, 0, enabled);
        drain(cleanup);
        if enabled {
            trapping(&s, 0, false);
            assert_eq!(take_exit(&s, 0, 1).reason_code(), 3);
            assert_eq!(s.handles[0].snapshot().pending_exit, 0);
        } else {
            trapping(&s, 0, true);
            assert_eq!(s.handles[0].snapshot().pending_exit, 3);
            assert_eq!(s.handles[0].snapshot().queued, 0);
        }
    }
}
#[test]
fn unlink_before_conversion_suppresses_exit_after_conversion_keeps_it() {
    for before in [false, true] {
        let mut s = Scene::new(2);
        trapping(&s, 0, true);
        connect(&s, 0, 1);
        let cleanup = s.retire(1, 2);
        if before {
            s.client.unlink(s.id(0), s.id(1));
        }
        drain(cleanup);
        s.client.unlink(s.id(0), s.id(1));
        assert_eq!(s.handles[0].snapshot().queued, usize::from(!before));
        if !before {
            assert_eq!(take_exit(&s, 0, 1).reason_code(), 2);
        }
    }
}
#[test]
fn relink_can_reactivate_old_exit_without_following_a_new_incarnation() {
    for missing_first in [false, true] {
        let mut s = Scene::new(2);
        trapping(&s, 0, true);
        connect(&s, 0, 1);
        let cleanup = s.retire(1, 2);
        s.client.unlink(s.id(0), s.id(1));
        connect(&s, 0, 1); // Missing trapped target creates a fresh pending noproc.
        if missing_first {
            assert_eq!(s.client.shared.links.service(8), 1);
        }
        drain(cleanup);
        s.client.shared.links.service(8);
        assert_eq!(
            take_exit(&s, 0, 1).reason_code(),
            if missing_first { 4 } else { 2 }
        );
        assert_eq!(s.handles[0].snapshot().queued, 0);
        assert_eq!(s.client.active_links(), 0);
    }
}
#[test]
fn absent_untrapped_link_is_a_synchronous_error_without_registration() {
    let mut s = Scene::new(2);
    drain(s.retire(1, 1));
    assert_eq!(s.client.link(s.id(0), s.id(1)), Err(LinkError::NoProcess));
    assert_eq!(
        (
            s.client.active_links(),
            s.handles[0].snapshot().pending_exit
        ),
        (0, 0)
    );
    assert_eq!(s.handles[0].snapshot().queued, 0);
    assert!(!s.client.shared.links.has_pending());
}
#[test]
fn trapped_absence_uses_current_trap_flag_and_can_be_unlinked() {
    for cancel in [false, true] {
        let mut s = Scene::new(2);
        drain(s.retire(1, 1));
        trapping(&s, 0, true);
        connect(&s, 0, 1);
        trapping(&s, 0, false);
        if cancel {
            s.client.unlink(s.id(0), s.id(1));
        }
        s.client.shared.links.service(8);
        assert_eq!(
            s.handles[0].snapshot().pending_exit,
            if cancel { 0 } else { 4 }
        );
        assert_eq!(s.client.active_links(), 0);
    }
}
#[test]
fn cancelling_absent_links_removes_work_and_coalesces_wakes() {
    let mut s = Scene::new(2);
    drain(s.retire(1, 1));
    trapping(&s, 0, true);
    for _ in 0..64 {
        connect(&s, 0, 1);
        s.client.unlink(s.id(0), s.id(1));
        assert!(!s.client.shared.links.has_pending());
        assert_eq!(s.client.active_links(), 0);
    }
    assert!(matches!(s.receiver.try_recv(), Ok(Command::LinkWork)));
    assert!(matches!(
        s.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    s.client.shared.links.acknowledge_wake();
    connect(&s, 0, 1);
    assert!(matches!(s.receiver.try_recv(), Ok(Command::LinkWork)));
}
#[test]
fn recycled_slot_cannot_receive_old_exit_or_unlink_new_pair() {
    let mut s = Scene::new(2);
    trapping(&s, 0, true);
    connect(&s, 0, 1);
    let old = s.id(1);
    let cleanup = s.retire(1, 2);
    let replacement = s.client.publish(prepared()).unwrap();
    let task = published(&s.receiver);
    task.metrics
        .runtime
        .set(Arc::downgrade(&s.client.shared))
        .unwrap();
    assert_eq!(old.slot, replacement.id().slot);
    assert_ne!(old, replacement.id());
    s.handles.push(replacement);
    s.tasks.push(Some(task));
    connect(&s, 0, 2);
    s.client.unlink(s.id(0), old);
    drain(cleanup);
    assert_eq!(s.client.active_links(), 1);
    assert_eq!(s.handles[0].snapshot().queued, 0);
    drain(s.retire(2, 1));
    assert_eq!(take_exit(&s, 0, 2).reason_code(), 1);
}
#[test]
fn simultaneous_endpoint_retirement_releases_shared_edge_once() {
    let mut s = Scene::new(2);
    connect(&s, 0, 1);
    let mut a = s.retire(0, 2);
    let mut b = s.retire(1, 2);
    assert!(a.step());
    assert!(b.step());
    assert_eq!(s.client.active_links(), 0);
    assert!(s.handles.iter().all(|h| h.snapshot().queued == 0));
}
#[test]
fn application_then_exit_then_down_share_queue_order() {
    let mut s = Scene::new(2);
    trapping(&s, 0, true);
    connect(&s, 0, 1);
    let reference = s.monitor(0, 1);
    s.client.send(s.id(0), packet(7, 70).0);
    drain(s.retire(1, 2));
    let Event::Message(message) = s.event(0) else {
        panic!("first message")
    };
    assert_eq!(message.into_words().read(0), 70);
    let Event::Exit(exit) = s.event(0) else {
        panic!("Exit before Down")
    };
    assert_eq!(exit.from(), s.id(1));
    let Event::Down(down) = s.event(0) else {
        panic!("Down after Exit")
    };
    assert_eq!(down.reference(), reference);
}
#[test]
fn link_and_monitor_retirement_share_one_eight_item_budget() {
    let mut s = Scene::new(11);
    for i in 1..11 {
        trapping(&s, i, true);
        connect(&s, 0, i);
        s.monitor(i, 0);
    }
    let mut cleanup = s.retire(0, 2);
    assert!(!cleanup.step());
    assert_eq!(s.client.active_links(), 2);
    assert_eq!(s.client.active_monitors(), 10);
    assert!(!cleanup.step());
    assert_eq!(s.client.active_links(), 0);
    // The two remaining links leave six slots for monitors in this batch.
    assert_eq!(s.client.active_monitors(), 4);
    assert!(cleanup.step());
    assert_eq!(
        (s.client.active_links(), s.client.active_monitors()),
        (0, 0)
    );
    let Event::Exit(_) = s.event(1) else {
        panic!("exit order")
    };
    let Event::Down(_) = s.event(1) else {
        panic!("down order")
    };
}
#[test]
fn finite_exit_selection_times_out_without_dropping_other_events() {
    let mut s = Scene::new(2);
    trapping(&s, 0, true);
    connect(&s, 0, 1);
    s.client.send(s.id(0), packet(7, 70).0);
    let task = s.tasks[0].as_ref().unwrap();
    let mut receive =
        mailbox::TimedExitReceive::new(task.metrics.clone(), task.budget.clone(), s.id(1), 0);
    assert!(matches!(
        poll(&mut receive, &task.budget, 1),
        Poll::Ready(ExitResult::Timeout)
    ));
    assert_eq!(s.handles[0].snapshot().queued, 1);
    drain(s.retire(1, 2));
    assert_eq!(take_exit(&s, 0, 1).reason_code(), 2);
    let Event::Message(message) = s.event(0) else {
        panic!("message retained")
    };
    assert_eq!(message.into_words().read(0), 70);
}
#[test]
fn actual_worker_retires_pending_fatal_without_polling_application() {
    let mut s = Scene::new(2);
    connect(&s, 0, 1);
    drain(s.retire(1, 2));
    let (tx, rx) = mpsc::channel();
    let mut task = s.tasks[0].take().unwrap();
    task.frame = Box::pin(async move {
        tx.send(()).unwrap();
    });
    s.client
        .shared
        .sender
        .send(Command::Spawn(task))
        .ok()
        .unwrap();
    let (_, empty) = mpsc::channel();
    let receiver = std::mem::replace(&mut s.receiver, empty);
    let shared = s.client.shared.clone();
    let worker = std::thread::spawn(move || crate::worker(receiver, 1, shared, true));
    // Channel disconnection is the destructor oracle, duration only a hang guard.
    let outcome = rx.recv_timeout(std::time::Duration::from_secs(10));
    s.client.close();
    worker.join().unwrap();
    assert!(matches!(outcome, Err(mpsc::RecvTimeoutError::Disconnected)));
    let snapshot = s.handles[0].snapshot();
    assert_eq!(
        (snapshot.polls, snapshot.terminal, snapshot.exit_reason),
        (0, 3, 2)
    );
}
#[test]
fn actual_worker_runs_peer_between_link_cleanup_batches() {
    let mut s = Scene::new(20);
    for i in 1..20 {
        trapping(&s, i, true);
        connect(&s, 0, i);
        s.monitor(i, 0);
    }
    let subject = s.handles[0].metrics.clone();
    let hub = s.client.shared.links.clone();
    let monitors = s.client.shared.monitors.clone();
    let (tx, rx) = mpsc::channel();
    s.tasks[1].as_mut().unwrap().frame = Box::pin(async move {
        tx.send((
            subject.cleanup_batches.load(Ordering::Acquire),
            subject.cleanup_event.load(Ordering::Acquire),
            hub.active(),
            monitors.active(),
        ))
        .unwrap();
    });
    s.handles[0].kill();
    for task in &mut s.tasks {
        s.client
            .shared
            .sender
            .send(Command::Spawn(task.take().unwrap()))
            .ok()
            .unwrap();
    }
    let (_, empty) = mpsc::channel();
    let receiver = std::mem::replace(&mut s.receiver, empty);
    let shared = s.client.shared.clone();
    let worker = std::thread::spawn(move || crate::worker(receiver, 1, shared, true));
    let observed = rx.recv_timeout(std::time::Duration::from_secs(10));
    s.client.close();
    worker.join().unwrap();
    assert_eq!(observed.unwrap(), (1, 0, 11, 19));
    assert_eq!(
        (s.client.active_links(), s.client.active_monitors()),
        (0, 0)
    );
}
