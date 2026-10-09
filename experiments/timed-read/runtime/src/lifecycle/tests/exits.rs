use super::monitors::{poll, Scene};
use super::*;

const REASONS: [ExitReason; 5] = [
    ExitReason::Normal,
    ExitReason::Killed,
    ExitReason::Failure,
    ExitReason::NoProcess,
    ExitReason::Kill,
];
fn next_exit(s: &Scene, recipient: usize, sender: usize, reason: u64) {
    let Event::Exit(exit) = s.event(recipient) else {
        panic!("expected Exit")
    };
    assert_eq!((exit.from(), exit.reason_code()), (s.id(sender), reason));
}
fn context(s: &Scene, i: usize) -> ProcessContext {
    ProcessContext {
        input: None,
        tag: 0,
        argument: 0,
        target: None,
        budget: s.tasks[i].as_ref().unwrap().budget.clone(),
        metrics: s.handles[i].metrics.clone(),
    }
}
#[test]
fn targeted_peer_reason_trap_matrix_preserves_link_until_death() {
    for reason in REASONS {
        for trap in [false, true] {
            let s = Scene::new(2);
            s.client.link(s.id(0), s.id(1)).unwrap();
            s.client.set_trap_exit(s.id(1), trap).unwrap();
            s.client.exit(s.id(0), s.id(1), reason).unwrap();
            assert_eq!(s.client.active_links(), 1);
            let state = s.handles[1].snapshot();
            if reason == ExitReason::Kill {
                assert!(state.kill_event > 0);
                assert_eq!((state.pending_exit, state.queued), (0, 0));
            } else if trap {
                next_exit(&s, 1, 0, reason.code());
                assert_eq!((state.kill_event, state.pending_exit), (0, 0));
            } else {
                assert_eq!(
                    state.pending_exit,
                    if reason == ExitReason::Normal {
                        0
                    } else {
                        reason.code()
                    }
                );
                assert_eq!((state.kill_event, state.queued), (0, 0));
            }
        }
    }
}
#[test]
fn legacy_self_target_reason_trap_matrix() {
    for reason in REASONS {
        for trap in [false, true] {
            let mut s = Scene::new(1);
            s.client.set_trap_exit(s.id(0), trap).unwrap();
            let ctx = context(&s, 0);
            let mut call = Box::pin(ctx.exit_to(s.id(0), reason));
            let result = poll(&mut call, &ctx.budget, 100);
            let fatal = !trap || reason == ExitReason::Kill;
            assert_eq!(result.is_pending(), fatal);
            drop(call);
            drop(ctx);
            if fatal {
                drain(s.retire(0, 1));
                let state = s.handles[0].snapshot();
                let code = if reason == ExitReason::Kill {
                    2
                } else {
                    reason.code()
                };
                assert_eq!(state.exit_reason, code);
                assert_eq!(
                    state.terminal,
                    if code == 1 {
                        1
                    } else if reason == ExitReason::Kill {
                        2
                    } else {
                        3
                    }
                );
            } else {
                next_exit(&s, 0, 0, reason.code());
            }
        }
    }
}
#[test]
fn self_exit_is_pending_for_every_reason_and_ignores_trapping() {
    for reason in REASONS {
        for trap in [false, true] {
            let mut s = Scene::new(1);
            s.client.set_trap_exit(s.id(0), trap).unwrap();
            let ctx = context(&s, 0);
            let mut call = Box::pin(ctx.self_exit(reason));
            assert!(poll(&mut call, &ctx.budget, 100).is_pending());
            assert_eq!(s.handles[0].snapshot().pending_exit, reason.code());
            drop(call);
            drop(ctx);
            drain(s.retire(0, 1));
            assert_eq!(s.handles[0].snapshot().exit_reason, reason.code());
        }
    }
}
#[test]
fn direct_kill_and_uncaught_kill_have_distinct_down_and_linked_exit() {
    for direct in [false, true] {
        let mut s = Scene::new(4);
        s.client.link(s.id(1), s.id(3)).unwrap();
        s.client.set_trap_exit(s.id(3), true).unwrap();
        let reference = s.monitor(2, 1);
        if direct {
            s.client.exit(s.id(0), s.id(1), ExitReason::Kill).unwrap();
        } else {
            let ctx = context(&s, 1);
            let mut call = Box::pin(ctx.self_exit(ExitReason::Kill));
            assert!(poll(&mut call, &ctx.budget, 100).is_pending());
        }
        drain(s.retire(1, 3));
        let code = if direct { 2 } else { 5 };
        next_exit(&s, 3, 1, code);
        let down = s.down(2, reference);
        assert_eq!(
            (down.target(), down.reference(), down.reason_code()),
            (s.id(1), reference, code)
        );
        assert_eq!(s.handles[3].snapshot().pending_exit, 0);
    }
}
#[test]
fn absent_reused_and_foreign_targets_are_noops_and_stale_origin_is_rejected() {
    let mut s = Scene::new(2);
    let old = s.id(1);
    drain(s.retire(1, 1));
    let replacement = s.client.publish(prepared()).unwrap();
    // Skip any retirement wake commands before taking the new frame.
    let replacement_task = loop {
        if let Command::Spawn(task) = s.receiver.try_recv().unwrap() {
            break task;
        }
    };
    assert_eq!(old.slot(), replacement.id().slot());
    assert_ne!(old, replacement.id());
    let foreign = Scene::new(1);
    let before = replacement.snapshot();
    for reason in REASONS {
        s.client.exit(s.id(0), old, reason).unwrap();
        s.client.exit(s.id(0), foreign.id(0), reason).unwrap();
        assert_eq!(
            s.client.exit(old, replacement.id(), reason),
            Err(ExitError::OriginNotLive)
        );
    }
    assert_eq!(replacement.snapshot(), before);
    assert_eq!(foreign.handles[0].snapshot().kill_event, 0);
    drain(retire(replacement_task, 1, &s.client.shared));
}
#[test]
fn first_fatal_receipt_is_irreversible_in_both_orders() {
    for first in [ExitReason::Kill, ExitReason::Failure] {
        let mut s = Scene::new(2);
        for reason in [
            first,
            ExitReason::Kill,
            ExitReason::Failure,
            ExitReason::Normal,
        ] {
            s.client.exit(s.id(0), s.id(1), reason).unwrap();
        }
        s.client.set_trap_exit(s.id(1), true).unwrap();
        drain(s.retire(1, 1));
        assert_eq!(
            s.handles[1].snapshot().exit_reason,
            if first == ExitReason::Kill { 2 } else { 3 }
        );
    }
}
#[test]
fn direct_exit_and_message_order_is_preserved_without_removing_links() {
    let s = Scene::new(2);
    s.client.set_trap_exit(s.id(1), true).unwrap();
    s.client.link(s.id(0), s.id(1)).unwrap();
    s.client.send(s.id(1), Envelope::new(10, vec![]).unwrap().0);
    s.client
        .exit(s.id(0), s.id(1), ExitReason::Failure)
        .unwrap();
    s.client.send(s.id(1), Envelope::new(20, vec![]).unwrap().0);
    assert!(matches!(s.event(1), Event::Message(m) if m.tag() == 10));
    next_exit(&s, 1, 0, 3);
    assert!(matches!(s.event(1), Event::Message(m) if m.tag() == 20));
    assert_eq!(s.client.active_links(), 1);
    s.client.unlink(s.id(0), s.id(1));
    assert_eq!(s.client.active_links(), 0);
}
#[test]
fn trap_is_sampled_at_receipt_and_later_change_keeps_converted_exit() {
    let s = Scene::new(2);
    s.client.set_trap_exit(s.id(1), true).unwrap();
    s.client.exit(s.id(0), s.id(1), ExitReason::Normal).unwrap();
    s.client.set_trap_exit(s.id(1), false).unwrap();
    next_exit(&s, 1, 0, 1);
    s.client.exit(s.id(0), s.id(1), ExitReason::Normal).unwrap();
    s.client.set_trap_exit(s.id(1), true).unwrap();
    assert_eq!(s.handles[1].snapshot().queued, 0);
}
#[test]
fn host_kill_and_direct_failure_race_keep_one_terminal_reason() {
    for _ in 0..16 {
        let mut s = Scene::new(3);
        let reference = s.monitor(2, 1);
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let c = s.client.clone();
        let sender = s.id(0);
        let target = s.id(1);
        let a = barrier.clone();
        let direct = std::thread::spawn(move || {
            a.wait();
            c.exit(sender, target, ExitReason::Failure).unwrap();
        });
        let handle = s.handles[1].clone();
        let b = barrier.clone();
        let killer = std::thread::spawn(move || {
            b.wait();
            handle.kill()
        });
        barrier.wait();
        direct.join().unwrap();
        let receipt = killer.join().unwrap();
        drain(s.retire(1, 1));
        let state = s.handles[1].snapshot();
        assert_eq!(
            state.exit_reason,
            if receipt == KillReceipt::Accepted {
                2
            } else {
                3
            }
        );
        assert_eq!(s.down(2, reference).reason_code(), state.exit_reason);
        assert_eq!(s.handles[2].snapshot().queued, 0);
    }
}

#[test]
fn synchronous_link_failure_cannot_overwrite_an_earlier_fatal_receipt() {
    for direct_first in [false, true] {
        let mut s = Scene::new(2);
        let absent = s.id(1);
        drain(s.retire(1, 1));
        if direct_first {
            s.client
                .exit(s.id(0), s.id(0), ExitReason::Failure)
                .unwrap();
        }
        let ctx = context(&s, 0);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ctx.link(absent))).is_err()
        );
        if !direct_first {
            s.client.exit(s.id(0), s.id(0), ExitReason::Kill).unwrap();
        }
        drop(ctx);
        drain(s.retire(0, 3));
        assert_eq!(
            s.handles[0].snapshot().exit_reason,
            if direct_first { 3 } else { 4 }
        );
    }
}
