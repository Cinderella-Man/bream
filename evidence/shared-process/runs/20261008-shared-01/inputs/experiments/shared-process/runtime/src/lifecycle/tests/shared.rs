use super::monitors::{poll, Scene};
use super::*;

#[test]
fn absent_and_foreign_delivery_consume_only_the_sent_reference() {
    let mut s = Scene::new(1);
    let other = Scene::new(1);
    let (payload, receipt) = SharedBytes::new(vec![41]).unwrap();
    s.client
        .send_shared(other.id(0), SharedEnvelope::new(1, payload.clone()));
    drain(s.retire(0, 2));
    s.client
        .send_shared(s.id(0), SharedEnvelope::new(1, payload.clone()));
    assert_eq!(payload.read(0), 41);
    assert_eq!(
        (
            receipt.snapshot().live_references,
            receipt.snapshot().references_dropped
        ),
        (1, 2)
    );
    drop(payload);
    assert_eq!(receipt.snapshot().physical_drops, 1);
}

#[test]
fn lookup_before_stop_cannot_retain_a_late_shared_delivery() {
    let mut s = Scene::new(1);
    let old = s.client.lookup(s.id(0)).unwrap();
    let (payload, receipt) = SharedBytes::new(vec![43]).unwrap();
    let pending = s.retire(0, 2);
    mailbox::deliver_event(
        &old.metrics,
        Event::Shared(SharedEnvelope::new(1, payload.clone())),
    );
    drain(pending);
    assert_eq!(
        (old.snapshot().queued, receipt.snapshot().live_references),
        (0, 1)
    );
    assert_eq!(payload.read(0), 43);
    drop(payload);
    assert_eq!(receipt.snapshot().physical_drops, 1);
}

#[test]
fn mixed_event_selection_preserves_message_down_exit_and_shared_order() {
    let mut s = Scene::new(2);
    s.client.set_trap_exit(s.id(0), true).unwrap();
    s.client.link(s.id(0), s.id(1)).unwrap();
    let reference = s.monitor(0, 1);
    let (payload, receipt) = SharedBytes::new(vec![47]).unwrap();
    s.client.send(s.id(0), packet(3, 30).0);
    s.client
        .send_shared(s.id(0), SharedEnvelope::new(8, payload.clone()));
    drain(s.retire(1, 1));
    s.client
        .send_shared(s.id(0), SharedEnvelope::new(9, payload));
    let task = s.tasks[0].as_ref().unwrap();
    let mut selected =
        mailbox::SharedReceive::new(task.metrics.clone(), task.budget.clone(), Some(9));
    assert!(poll(&mut selected, &task.budget, 1).is_pending());
    assert!(!s.handles[0].snapshot().waiting);
    let Poll::Ready(message) = poll(&mut selected, &task.budget, 100) else {
        panic!("missing share")
    };
    assert_eq!(message.into_bytes().read(0), 47);
    assert!(matches!(s.event(0), Event::Message(_)));
    assert!(matches!(s.event(0), Event::Shared(_)));
    let Event::Exit(exit) = s.event(0) else {
        panic!("missing Exit")
    };
    assert_eq!(exit.from(), s.id(1));
    let Event::Down(down) = s.event(0) else {
        panic!("missing Down")
    };
    assert_eq!(down.reference(), reference);
    assert_eq!(receipt.snapshot().physical_drops, 1);
}

#[test]
fn zero_shared_timeout_keeps_nonmatching_payload_for_later_receive() {
    let s = Scene::new(1);
    let (payload, receipt) = SharedBytes::new(vec![53]).unwrap();
    s.client
        .send_shared(s.id(0), SharedEnvelope::new(8, payload));
    let task = s.tasks[0].as_ref().unwrap();
    let mut timed =
        mailbox::TimedSharedReceive::new(task.metrics.clone(), task.budget.clone(), Some(9), 0);
    assert!(matches!(
        poll(&mut timed, &task.budget, 10),
        Poll::Ready(SharedReceiveResult::Timeout)
    ));
    assert_eq!(receipt.snapshot().live_references, 1);
    let Event::Shared(message) = s.event(0) else {
        panic!("preserved share absent")
    };
    assert_eq!(message.into_bytes().read(0), 53);
    assert_eq!(receipt.snapshot().physical_drops, 1);
}

#[test]
fn shared_queue_cleanup_releases_at_most_eight_envelopes_per_step() {
    let mut s = Scene::new(1);
    let (survivor, receipt) = SharedBytes::new(vec![59]).unwrap();
    for tag in 0..19 {
        s.client
            .send_shared(s.id(0), SharedEnvelope::new(tag, survivor.clone()));
    }
    let mut cleanup = s.retire(0, 2);
    for (remaining, complete) in [(12, false), (4, false), (1, true)] {
        assert_eq!(cleanup.step(), complete);
        assert_eq!(receipt.snapshot().live_references, remaining);
        assert_eq!(receipt.snapshot().physical_drops, 0);
        assert_eq!(survivor.read(0), 59);
    }
    assert_eq!(s.handles[0].snapshot().cleanup_messages, 19);
    drop(survivor);
    let result = receipt.snapshot();
    assert_eq!(
        (
            result.references_created,
            result.references_dropped,
            result.physical_drops
        ),
        (20, 20, 1)
    );
}

#[test]
fn typed_words_receive_skips_shared_payload_without_consuming_it() {
    let s = Scene::new(1);
    let (payload, receipt) = SharedBytes::new(vec![61]).unwrap();
    s.client
        .send_shared(s.id(0), SharedEnvelope::new(7, payload));
    s.client.send(s.id(0), packet(7, 70).0);
    let task = s.tasks[0].as_ref().unwrap();
    let mut receive = mailbox::Receive::new(task.metrics.clone(), task.budget.clone(), Some(7));
    assert!(poll(&mut receive, &task.budget, 100).is_ready());
    assert_eq!(receipt.snapshot().live_references, 1);
    assert!(matches!(s.event(0), Event::Shared(_)));
    assert_eq!(receipt.snapshot().physical_drops, 1);
}
