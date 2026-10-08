extern crate bream_busy_runtime as actual_runtime;
extern crate self as bream_busy_runtime;
extern crate self as core;
extern crate self as std;
extern crate std as actual_std;
pub struct ProcessId;
pub struct Envelope;
pub struct MonitorRef;
pub struct Down;
pub struct Event;
pub struct EventResult;
pub struct DownResult;
pub mod option {
    pub struct Option;
}
pub mod primitive {
    #[allow(non_camel_case_types)]
    pub type u64 = u8;
}
use bream_busy_macros::process_module;
#[process_module]
mod checked {
    fn target() -> Pid {
        return self_id();
    }
    fn acquire() -> MonitorRef {
        return monitor(target());
    }
    fn selected(reference: MonitorRef) -> MonitorRef {
        mark(20);
        return reference;
    }
    fn delay() -> u64 {
        mark(21);
        return 0;
    }
    fn describe(down: Down) -> u64 {
        let reference = down_ref(down);
        let target_id = down_target(down);
        demonitor(reference);
        return down_reason(down);
    }
    fn wait_for(reference: MonitorRef) -> Down {
        return receive_down(reference);
    }
    fn run() {
        let reference = acquire();
        let copied = reference;
        if copied != reference {
            result(999);
            return;
        }
        match receive_down_for(selected(reference), delay()) {
            DownResult::Down(unexpected) => {
                result(describe(unexpected));
                return;
            }
            DownResult::Timeout => {}
        }
        demonitor(selected(reference));
        demonitor_flush(selected(copied));
        send(target(), 1000, owned_input());
        match receive_event() {
            Event::Message(first_message) => {
                let moved_words = message_words(first_message);
                send(target(), 1001, moved_words);
            }
            Event::Down(first_down) => {
                result(describe(first_down));
                return;
            }
            Event::Shared(unexpected_shared_1) => {
                result(999);
                return;
            }
            Event::Exit(first_exit) => {
                result(9000 + exit_reason(first_exit));
                return;
            }
        }
        match receive_event_for(delay()) {
            EventResult::Event(event) => match event {
                Event::Message(message) => {
                    let tag = message_tag(message);
                    let words = message_words(message);
                    result(tag + read(words, 0));
                }
                Event::Down(down) => {
                    result(describe(down));
                }
                Event::Shared(unexpected_shared_2) => {
                    result(999);
                    return;
                }
                Event::Exit(exit) => {
                    result(9000 + exit_reason(exit));
                }
            },
            EventResult::Timeout => {
                result(999);
            }
        }
    }
}
fn main() {
    let rt = actual_runtime::Runtime::paused(1);
    let h = rt.spawn(checked::program(), actual_std::vec![42]).unwrap();
    rt.start();
    let deadline = actual_std::time::Instant::now() + actual_std::time::Duration::from_secs(30);
    loop {
        let s = h.snapshot();
        if s.cleanup_event > 0 {
            assert_eq!(
                (s.terminal, s.result, s.drops, s.freed_bytes),
                (1, 1043, 1, 8)
            );
            assert_eq!(s.receives, 2);
            break;
        }
        assert!(
            actual_std::time::Instant::now() < deadline,
            "correctness watchdog"
        );
        actual_std::thread::yield_now();
    }
    rt.shutdown();
    actual_std::println!("SHADOW_MONITOR_PASSED");
}
