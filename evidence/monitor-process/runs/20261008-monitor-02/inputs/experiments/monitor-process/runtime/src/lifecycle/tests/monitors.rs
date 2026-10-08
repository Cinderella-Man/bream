use super::*;

struct Scene {
    client: RuntimeClient,
    handles: Vec<ProcessHandle>,
    receiver: mpsc::Receiver<Command>,
    tasks: Vec<Option<Task>>,
}
impl Scene {
    fn new(count: usize) -> Self {
        let (client, receiver) = fixture();
        *client.shared.registry.lock().unwrap() = Registry::new(count + 2);
        let mut handles = Vec::new();
        let mut tasks = Vec::new();
        for _ in 0..count {
            let handle = client.publish(prepared()).unwrap();
            let task = published(&receiver);
            task.metrics
                .runtime
                .set(Arc::downgrade(&client.shared))
                .unwrap();
            handles.push(handle);
            tasks.push(Some(task));
        }
        Self {
            client,
            handles,
            tasks,
            receiver,
        }
    }
    fn id(&self, i: usize) -> ProcessId {
        self.handles[i].id()
    }
    fn monitor(&self, from: usize, to: usize) -> MonitorRef {
        self.client.monitor(self.id(from), self.id(to)).unwrap()
    }
    fn retire(&mut self, i: usize, reason: u8) -> Cleanup {
        retire(self.tasks[i].take().unwrap(), reason, &self.client.shared)
    }
    fn event(&self, i: usize) -> Event {
        let task = self.tasks[i].as_ref().unwrap();
        let mut receive = mailbox::EventReceive::new(task.metrics.clone(), task.budget.clone());
        let Poll::Ready(event) = poll(&mut receive, &task.budget, 100) else {
            panic!("event absent")
        };
        event
    }
    fn down(&self, i: usize, reference: MonitorRef) -> Down {
        let task = self.tasks[i].as_ref().unwrap();
        let mut receive =
            mailbox::DownReceive::new(task.metrics.clone(), task.budget.clone(), reference);
        let Poll::Ready(down) = poll(&mut receive, &task.budget, 100) else {
            panic!("Down absent")
        };
        down
    }
}
impl Drop for Scene {
    fn drop(&mut self) {
        for task in &mut self.tasks {
            if let Some(task) = task.take() {
                drain(retire(task, 2, &self.client.shared));
            }
        }
        while self.client.shared.monitors.has_pending() {
            self.client.shared.monitors.service(8);
        }
        assert_eq!(self.client.active_monitors(), 0);
    }
}
fn poll<F: Future + Unpin>(future: &mut F, budget: &Budget, allowance: u32) -> Poll<F::Output> {
    budget.remaining.store(allowance, Ordering::Relaxed);
    let waker = Waker::from(Arc::new(NoopWake));
    Pin::new(future).poll(&mut Context::from_waker(&waker))
}

#[test]
fn self_monitor_allocates_distinct_references_without_relationships() {
    let scene = Scene::new(1);
    let a = scene.monitor(0, 0);
    let b = scene.monitor(0, 0);
    assert_ne!(a, b);
    assert_eq!(scene.client.active_monitors(), 0);
    assert!(!scene.client.demonitor_info(scene.id(0), a));
    assert_eq!(scene.client.shared.monitors.service(8), 0);
    assert_eq!(scene.handles[0].snapshot().queued, 0);
}
#[test]
fn independent_monitors_report_stopped_exact_incarnation_once() {
    let mut scene = Scene::new(2);
    let a = scene.monitor(0, 1);
    let b = scene.monitor(0, 1);
    assert_ne!(a, b);
    assert!(!scene.client.demonitor_info(scene.id(1), a));
    let mut cleanup = scene.retire(1, 3);
    assert_eq!(scene.handles[0].snapshot().queued, 0);
    assert!(!scene.client.is_live(scene.id(1)));
    assert!(scene.handles[1].snapshot().stop_event > 0);
    assert!(cleanup.step());
    for reference in [a, b] {
        let down = scene.down(0, reference);
        assert_eq!(
            (down.reference(), down.target(), down.reason_code()),
            (reference, scene.id(1), 3)
        );
        assert!(!scene.client.demonitor_info(scene.id(0), reference));
    }
    assert_eq!(scene.handles[0].snapshot().queued, 0);
}
#[test]
fn cancellation_suppresses_detached_down_before_mailbox_conversion() {
    let mut scene = Scene::new(2);
    let reference = scene.monitor(0, 1);
    let cleanup = scene.retire(1, 1); // terminal committed; notification not converted
    assert!(scene.client.demonitor_info(scene.id(0), reference));
    assert!(!scene.client.demonitor_info(scene.id(0), reference));
    drain(cleanup);
    assert_eq!(scene.handles[0].snapshot().queued, 0);
}
#[test]
fn plain_cancel_leaves_converted_down_while_flush_preserves_other_kinds() {
    let mut scene = Scene::new(3);
    let first = scene.monitor(0, 1);
    let second = scene.monitor(0, 2);
    scene.client.send(scene.id(0), packet(7, 70).0);
    drain(scene.retire(1, 1));
    scene.client.send(scene.id(0), packet(8, 80).0);
    drain(scene.retire(2, 2));
    assert!(!scene.client.demonitor_info(scene.id(0), first));
    assert_eq!(scene.handles[0].snapshot().queued, 4);
    let task = scene.tasks[0].as_ref().unwrap();
    let mut flush = mailbox::Flush::new(task.metrics.clone(), task.budget.clone(), first);
    assert!(poll(&mut flush, &task.budget, 1).is_pending());
    assert!(!scene.handles[0].snapshot().waiting);
    assert!(poll(&mut flush, &task.budget, 1).is_ready());
    assert_eq!(scene.handles[0].snapshot().queued, 3);
    for expected in [70, 80] {
        let Event::Message(message) = scene.event(0) else {
            panic!("message order")
        };
        assert_eq!(message.into_words().read(0), expected);
    }
    let Event::Down(down) = scene.event(0) else {
        panic!("other monitor retained")
    };
    assert_eq!(down.reference(), second);
}
#[test]
fn down_selection_preserves_application_and_other_monitor_order() {
    let mut scene = Scene::new(3);
    let a = scene.monitor(0, 1);
    let b = scene.monitor(0, 2);
    scene.client.send(scene.id(0), packet(9, 90).0);
    drain(scene.retire(1, 1));
    scene.client.send(scene.id(0), packet(8, 80).0);
    drain(scene.retire(2, 1));
    assert_eq!(scene.down(0, b).reference(), b);
    let Event::Message(first) = scene.event(0) else {
        panic!("first app")
    };
    assert_eq!(first.into_words().read(0), 90);
    let Event::Down(second) = scene.event(0) else {
        panic!("older Down")
    };
    assert_eq!(second.reference(), a);
    let Event::Message(third) = scene.event(0) else {
        panic!("later app")
    };
    assert_eq!(third.into_words().read(0), 80);
}
#[test]
fn absent_target_down_can_be_cancelled_before_conversion() {
    let mut scene = Scene::new(2);
    drain(scene.retire(1, 2));
    let cancelled = scene.monitor(0, 1);
    let retained = scene.monitor(0, 1);
    assert!(scene.client.demonitor_info(scene.id(0), cancelled));
    assert_eq!(scene.client.shared.monitors.service(8), 1);
    let down = scene.down(0, retained);
    assert_eq!((down.target(), down.reason_code()), (scene.id(1), 4));
    assert_eq!(scene.handles[0].snapshot().queued, 0);
}
#[test]
fn reuse_cannot_redirect_old_reference_or_absent_target() {
    let mut scene = Scene::new(2);
    let old = scene.id(1);
    let old_ref = scene.monitor(0, 1);
    let old_cleanup = scene.retire(1, 1);
    let replacement = scene.client.publish(prepared()).unwrap();
    let new_task = published(&scene.receiver);
    assert_eq!(replacement.id().slot, old.slot);
    assert_ne!(replacement.id(), old);
    scene.handles.push(replacement);
    scene.tasks.push(Some(new_task));
    let fresh = scene.monitor(0, 2);
    let absent = scene.client.monitor(scene.id(0), old).unwrap();
    scene.client.shared.monitors.service(8);
    drain(old_cleanup);
    assert_eq!(scene.down(0, absent).reason_code(), 4);
    assert_eq!(scene.down(0, old_ref).reason_code(), 1);
    assert!(scene.client.is_live(scene.id(2)));
    drain(scene.retire(2, 2));
    assert_eq!(scene.down(0, fresh).target(), scene.id(2));
}
#[test]
fn observer_death_releases_many_registrations_in_bounded_batches() {
    let mut scene = Scene::new(2);
    for _ in 0..19 {
        scene.monitor(0, 1);
    }
    let mut cleanup = scene.retire(0, 2);
    assert!(!cleanup.step());
    assert_eq!(scene.client.active_monitors(), 11);
    assert!(!cleanup.step());
    assert_eq!(scene.client.active_monitors(), 3);
    assert!(cleanup.step());
    assert_eq!(scene.client.active_monitors(), 0);
    assert_eq!(scene.handles[0].snapshot().cleanup_batches, 3);
    drain(scene.retire(1, 1));
}
#[test]
fn crossing_observer_and_target_death_settle_each_registration_once() {
    for observer_first in [false, true] {
        let mut scene = Scene::new(2);
        for _ in 0..9 {
            scene.monitor(0, 1);
        }
        let (first, second) = if observer_first { (0, 1) } else { (1, 0) };
        let mut a = scene.retire(first, 2);
        let mut b = scene.retire(second, 2);
        assert!(!a.step());
        assert!(!b.step());
        assert!(a.step());
        assert!(b.step());
        assert_eq!(scene.client.active_monitors(), 0);
        assert_eq!(scene.handles[0].snapshot().queued, 0);
    }
}
#[test]
fn down_is_observable_before_final_tracked_cleanup() {
    let mut scene = Scene::new(2);
    let reference = scene.monitor(0, 1);
    let mut receipts = Vec::new();
    for _ in 0..25 {
        let (message, receipt) = packet(7, 70);
        scene.client.send(scene.id(1), message);
        receipts.push(receipt);
    }
    let mut cleanup = scene.retire(1, 2);
    assert!(!cleanup.step());
    let down = scene.down(0, reference);
    assert_eq!(down.reason_code(), 2);
    let snapshot = scene.handles[1].snapshot();
    assert!(snapshot.stop_event > 0 && snapshot.cleanup_event == 0 && !snapshot.polling);
    assert!(!scene.client.is_live(down.target()));
    assert_eq!(receipts.iter().filter(|r| r.drops() == 1).count(), 8);
    drain(cleanup);
    assert!(receipts.iter().all(|r| r.drops() == 1));
}
#[test]
fn finite_down_receive_times_out_then_late_down_stays_available() {
    let mut scene = Scene::new(2);
    let reference = scene.monitor(0, 1);
    let task = scene.tasks[0].as_ref().unwrap();
    let mut receive =
        mailbox::TimedDownReceive::new(task.metrics.clone(), task.budget.clone(), reference, 5);
    assert!(poll(&mut receive, &task.budget, 1).is_pending());
    // Publish logical time directly so the test controls the service boundary.
    scene.client.shared.clock.advance_to(5).unwrap();
    assert_eq!(scene.client.shared.clock.service(8), 1);
    assert!(matches!(
        poll(&mut receive, &task.budget, 1),
        Poll::Ready(DownResult::Timeout)
    ));
    drain(scene.retire(1, 1));
    assert_eq!(scene.down(0, reference).reason_code(), 1);
}
#[test]
fn down_wakes_waiter_and_absent_service_is_bounded() {
    let mut scene = Scene::new(2);
    drain(scene.retire(1, 1));
    let mut refs = Vec::new();
    for _ in 0..19 {
        refs.push(scene.monitor(0, 1));
    }
    let task = scene.tasks[0].as_ref().unwrap();
    let mut receive = mailbox::DownReceive::new(task.metrics.clone(), task.budget.clone(), refs[0]);
    assert!(poll(&mut receive, &task.budget, 1).is_pending());
    assert!(scene.handles[0].snapshot().waiting);
    assert_eq!(scene.client.shared.monitors.service(8), 8);
    assert!(!scene.handles[0].snapshot().waiting);
    assert_eq!(scene.client.active_monitors(), 11);
    assert!(poll(&mut receive, &task.budget, 1).is_ready());
    assert_eq!(scene.client.shared.monitors.service(8), 8);
    assert_eq!(scene.client.shared.monitors.service(8), 3);
    assert!(!scene.client.shared.monitors.has_pending());
}

#[test]
fn reference_exhaustion_fails_without_registration_or_poison() {
    let scene = Scene::new(2);
    scene.client.shared.monitors.exhaust_references();
    for _ in 0..2 {
        assert_eq!(
            scene.client.monitor(scene.id(0), scene.id(1)),
            Err(MonitorError::Exhausted)
        );
        assert_eq!(scene.client.active_monitors(), 0);
        assert_eq!(scene.handles[0].snapshot().queued, 0);
    }
}
#[test]
fn cancelled_absent_work_is_removed_and_wakes_are_coalesced() {
    let mut scene = Scene::new(2);
    drain(scene.retire(1, 1));
    for _ in 0..64 {
        let reference = scene.monitor(0, 1);
        assert!(scene.client.demonitor_info(scene.id(0), reference));
        assert!(!scene.client.shared.monitors.has_pending());
        assert_eq!(scene.client.active_monitors(), 0);
    }
    assert!(matches!(
        scene.receiver.try_recv(),
        Ok(Command::MonitorWork)
    ));
    assert!(matches!(
        scene.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    scene.client.shared.monitors.acknowledge_wake();
    scene.monitor(0, 1);
    assert!(matches!(
        scene.receiver.try_recv(),
        Ok(Command::MonitorWork)
    ));
}
#[test]
fn self_monitors_do_not_enqueue_worker_commands() {
    let scene = Scene::new(1);
    for _ in 0..64 {
        scene.monitor(0, 0);
    }
    assert!(matches!(
        scene.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

#[test]
fn actual_worker_polls_peer_between_monitor_cleanup_batches() {
    let mut scene = Scene::new(2);
    for _ in 0..19 {
        scene.monitor(0, 1);
    }
    let subject = scene.handles[0].metrics.clone();
    let (seen_tx, seen_rx) = mpsc::channel();
    // An internal scheduler fixture, not an admitted application escape hatch.
    scene.tasks[1].as_mut().unwrap().frame = Box::pin(async move {
        let batches = subject.cleanup_batches.load(Ordering::Acquire);
        let reclaimed = subject.cleanup_event.load(Ordering::Acquire);
        seen_tx.send((batches, reclaimed)).unwrap();
    });
    scene.handles[0].kill();
    for task in &mut scene.tasks {
        scene
            .client
            .shared
            .sender
            .send(Command::Spawn(task.take().unwrap()))
            .ok()
            .unwrap();
    }
    let (_, empty_receiver) = mpsc::channel();
    let receiver = std::mem::replace(&mut scene.receiver, empty_receiver);
    let shared = scene.client.shared.clone();
    let worker = std::thread::spawn(move || super::super::super::worker(receiver, 1, shared, true));
    // Hang watchdog only; the oracle is the logical cleanup stage at peer poll.
    let observation = seen_rx.recv_timeout(std::time::Duration::from_secs(10));
    scene.client.close();
    worker.join().unwrap();
    assert_eq!(observation.unwrap(), (1, 0));
    assert_eq!(scene.client.active_monitors(), 0);
    assert!(scene.handles.iter().all(|h| h.snapshot().cleanup_event > 0));
}
