use super::*;
use bream_timed_runtime::{SharedBytes, SharedEnvelope};

#[process_module]
mod self_case {
    fn choose_reason() -> ExitReason {
        if input_tag() == 1 {
            return ExitReason::Normal;
        }
        if input_tag() == 2 {
            return ExitReason::Killed;
        }
        if input_tag() == 3 {
            return ExitReason::Failure;
        }
        if input_tag() == 4 {
            return ExitReason::NoProcess;
        }
        return ExitReason::Kill;
    }
    fn stop(reason: ExitReason, words: Words, bytes: SharedBytes) {
        mark(20);
        self_exit(reason);
        mark(999);
        result(len(words) + shared_len(bytes));
    }
    fn run() {
        trap_exit(input_argument() == 1 || input_argument() == 3);
        let words = owned_input();
        let bytes = shared_bytes(receive_shared(7));
        mark(10);
        let gate = receive(99);
        if input_argument() < 2 {
            stop(choose_reason(), words, bytes);
            mark(999);
        } else {
            mark(20);
            exit_to(self_id(), choose_reason());
            mark(30);
            let notification = receive_exit(self_id());
            if exit_from(notification) != self_id() || len(words) + shared_len(bytes) != 6 {
                result(999);
                return;
            }
            result(exit_reason(notification));
        }
    }
}
#[process_module]
mod target_case {
    fn run() {
        trap_exit(input_argument() == 1);
        let words = owned_input();
        let bytes = shared_bytes(receive_shared(7));
        mark(10);
        let gate = receive(99);
        if input_argument() == 1 {
            match receive_event() {
                Event::Exit(notification) => {
                    if exit_from(notification) == self_id() {
                        result(999);
                        return;
                    }
                    result(exit_reason(notification));
                }
                Event::Message(unexpected) => {
                    result(999);
                    return;
                }
                Event::Shared(unexpected_shared) => {
                    result(999);
                    return;
                }
                Event::Down(unexpected_down) => {
                    result(999);
                    return;
                }
            }
        } else {
            result(0);
        }
        if len(words) + shared_len(bytes) != 6 {
            result(999);
            return;
        }
        mark(30);
    }
}
#[process_module]
mod sender_case {
    fn choose_reason() -> ExitReason {
        if input_tag() == 1 {
            return ExitReason::Normal;
        }
        if input_tag() == 2 {
            return ExitReason::Killed;
        }
        if input_tag() == 3 {
            return ExitReason::Failure;
        }
        if input_tag() == 4 {
            return ExitReason::NoProcess;
        }
        return ExitReason::Kill;
    }
    fn run() {
        exit_to(input_target(), choose_reason());
        mark(30);
        result(1);
    }
}

#[process_module]
mod busy_target {
    fn run() {
        let words = owned_input();
        let bytes = shared_bytes(receive_shared(7));
        mark(10);
        let mut step: u64 = 0;
        loop {
            step += read(words, 0) + shared_read(bytes, 0);
            observe(step);
        }
    }
}
fn busy_controls(allowance: u32) -> Vec<Vec<u64>> {
    let mut rows = Vec::new();
    for code in [3_u64, 5] {
        let rt = Runtime::new(allowance);
        let actor = rt.spawn(busy_target::program(), vec![11, 13]).unwrap();
        let (bytes, receipt) = SharedBytes::new(vec![2, 3, 5, 7]).unwrap();
        rt.client()
            .send_shared(actor.id(), SharedEnvelope::new(7, bytes.clone()));
        wait(&actor, |s| s.stage == 10 && s.progress > 0);
        for _ in 0..19 {
            rt.client()
                .send_shared(actor.id(), SharedEnvelope::new(88, bytes.clone()));
        }
        let peer = rt.spawn(peer::program(), vec![]).unwrap();
        assert_eq!(done(&peer, 1).result, 9);
        let sender = rt
            .spawn_to(
                sender_case::program(),
                Input {
                    words: vec![],
                    tag: code,
                    argument: 0,
                },
                actor.id(),
            )
            .unwrap();
        assert_eq!(done(&sender, 1).result, 1);
        let stopped = done(&actor, if code == 5 { 2 } else { 3 });
        assert_eq!(
            (
                stopped.exit_reason,
                stopped.stage,
                stopped.receives,
                stopped.drops,
                stopped.cleanup_messages,
                stopped.cleanup_batches
            ),
            (if code == 5 { 2 } else { 3 }, 10, 1, 1, 19, 3)
        );
        let after_peer = rt.spawn(peer::program(), vec![]).unwrap();
        assert_eq!(done(&after_peer, 1).result, 9);
        frozen_application(&stopped, &actor.snapshot());
        assert_eq!(
            (
                receipt.snapshot().live_references,
                receipt.snapshot().references_dropped
            ),
            (1, 20)
        );
        drop(bytes);
        let ownership = receipt.snapshot();
        assert_eq!(
            (
                ownership.physical_drops,
                ownership.references_created,
                ownership.references_dropped
            ),
            (1, 21, 21)
        );
        rows.push(vec![
            code,
            u64::from(stopped.terminal),
            stopped.exit_reason,
            stopped.stage,
            stopped.receives,
            stopped.cleanup_messages,
            stopped.cleanup_batches,
        ]);
        rt.shutdown();
    }
    rows
}
fn stale_control(allowance: u32) {
    let rt = Runtime::with_capacity(allowance, 2);
    let old = rt.spawn(waiter::program(), vec![]).unwrap();
    wait(&old, |s| s.waiting);
    old.kill();
    done(&old, 2);
    let replacement = rt.spawn(waiter::program(), vec![]).unwrap();
    wait(&replacement, |s| s.waiting);
    assert_eq!(replacement.id().slot(), old.id().slot());
    assert_ne!(replacement.id(), old.id());
    let before = replacement.snapshot();
    let sender = rt
        .spawn_to(
            sender_case::program(),
            Input {
                words: vec![],
                tag: 5,
                argument: 0,
            },
            old.id(),
        )
        .unwrap();
    assert_eq!(done(&sender, 1).result, 1);
    assert_eq!(replacement.snapshot(), before);
    rt.shutdown();
    done(&replacement, 2);
}

pub(super) fn run(allowance: u32) {
    let busy = busy_controls(allowance);
    stale_control(allowance);
    let mut rows = Vec::new();
    let mut cleanup_rows = Vec::new();
    for mode in 1..=3_u64 {
        for reason in 1..=5_u64 {
            for trap in 0..=1_u64 {
                let rt = Runtime::new(allowance);
                let actor = rt
                    .spawn_input(
                        if mode == 3 {
                            target_case::program()
                        } else {
                            self_case::program()
                        },
                        Input {
                            words: vec![11, 13],
                            tag: reason,
                            argument: if mode == 2 { trap + 2 } else { trap },
                        },
                    )
                    .unwrap();
                let (bytes, receipt) = SharedBytes::new(vec![2, 3, 5, 7]).unwrap();
                rt.client()
                    .send_shared(actor.id(), SharedEnvelope::new(7, bytes.clone()));
                wait(&actor, |s| s.stage == 10 && s.waiting);
                let mut queue_count = 0;
                if mode != 3 {
                    for _ in 0..19 {
                        rt.client()
                            .send_shared(actor.id(), SharedEnvelope::new(88, bytes.clone()));
                        queue_count += 1;
                    }
                }
                let terminal_signal = mode == 1
                    || (mode == 2 && (trap == 0 || reason == 5))
                    || (mode == 3 && (reason == 5 || (trap == 0 && reason != 1)));
                if mode == 3 {
                    let sender = rt
                        .spawn_to(
                            sender_case::program(),
                            Input {
                                words: vec![],
                                tag: reason,
                                argument: 0,
                            },
                            actor.id(),
                        )
                        .unwrap();
                    let completed = done(&sender, 1);
                    assert_eq!((completed.stage, completed.result), (30, 1));
                }
                let mut gate_receipt = None;
                if mode != 3 || !terminal_signal {
                    let (gate, r) = packet(99, vec![9]);
                    rt.client().send(actor.id(), gate);
                    gate_receipt = Some(r);
                }
                let effective = if mode != 1 && reason == 5 { 2 } else { reason };
                let expected_terminal = if !terminal_signal || effective == 1 {
                    1
                } else if mode != 1 && reason == 5 {
                    2
                } else {
                    3
                };
                let stopped = done(&actor, expected_terminal);
                assert_eq!(
                    stopped.exit_reason,
                    if terminal_signal { effective } else { 1 }
                );
                assert_eq!(
                    stopped.stage,
                    if terminal_signal {
                        if mode == 3 {
                            10
                        } else {
                            20
                        }
                    } else {
                        30
                    }
                );
                assert_eq!(
                    stopped.result,
                    if terminal_signal || trap == 0 {
                        0
                    } else {
                        reason
                    }
                );
                assert_eq!(
                    (stopped.drops, stopped.live_bytes, stopped.freed_bytes),
                    (1, 0, 16)
                );
                assert_eq!(stopped.cleanup_messages, queue_count);
                if queue_count == 19 {
                    assert_eq!(stopped.cleanup_batches, 3);
                }
                if let Some(r) = gate_receipt {
                    released(&[r], 8);
                }
                let before = receipt.snapshot();
                assert_eq!(
                    (
                        before.live_references,
                        before.references_created,
                        before.references_dropped
                    ),
                    (1, queue_count + 2, queue_count + 1)
                );
                assert_eq!(before.physical_drops, 0);
                drop(bytes);
                let after = receipt.snapshot();
                assert_eq!(
                    (
                        after.live_bytes,
                        after.freed_bytes,
                        after.physical_drops,
                        after.live_references
                    ),
                    (0, 4, 1, 0)
                );
                let peer = rt.spawn(peer::program(), vec![]).unwrap();
                assert_eq!(done(&peer, 1).result, 9);
                frozen_application(&stopped, &actor.snapshot());
                assert_eq!(
                    (
                        rt.client().live_count(),
                        rt.client().active_links(),
                        rt.client().active_monitors()
                    ),
                    (0, 0, 0)
                );
                let outcome = if terminal_signal {
                    2
                } else if trap == 1 {
                    1
                } else {
                    0
                };
                let observed = if terminal_signal {
                    effective
                } else if trap == 1 {
                    reason
                } else {
                    0
                };
                let observed_outcome = if stopped.stage == 30 {
                    if trap == 1 {
                        1
                    } else {
                        0
                    }
                } else {
                    2
                };
                let observed_reason = if observed_outcome == 2 {
                    stopped.exit_reason
                } else {
                    stopped.result
                };
                assert_eq!((observed_outcome, observed_reason), (outcome, observed));
                rows.push(vec![
                    mode,
                    reason,
                    trap,
                    observed_outcome,
                    observed_reason,
                    u64::from(mode == 3 || stopped.stage == 30),
                ]);
                cleanup_rows.push(vec![
                    mode,
                    reason,
                    trap,
                    stopped.stage,
                    stopped.drops,
                    stopped.cleanup_messages,
                    stopped.cleanup_batches,
                    after.references_created,
                    after.references_dropped,
                    after.physical_drops,
                ]);
                rt.shutdown();
            }
        }
    }
    println!("{{\"kind\":\"exit_controls\",\"passed\":true,\"speed_tests\":false,\"allowance\":{},\"matrix\":{:?},\"cleanup\":{:?},\"busy\":{:?},\"stale_target_ignored\":true}}", allowance, rows, cleanup_rows, busy);
}
