extern crate bream_timed_runtime as actual_runtime;
extern crate self as bream_timed_runtime;
extern crate self as core;
extern crate self as std;
extern crate std as actual_std;
pub struct ExitReason;
pub struct ProcessId;
pub struct Exit;
pub mod primitive {
    #[allow(non_camel_case_types)]
    pub type u64 = u8;
    #[allow(non_camel_case_types)]
    pub type bool = u8;
}
use bream_timed_macros::process_module;
#[process_module]
mod checked {
    fn target_once() -> Pid {
        send(self_id(), 7, owned_input());
        return self_id();
    }
    fn reason_once() -> ExitReason {
        let words = message_words(receive(7));
        mark(read(words, 0));
        return ExitReason::Normal;
    }
    fn run() {
        trap_exit(true);
        exit_to(target_once(), reason_once());
        let notification = receive_exit(self_id());
        if exit_from(notification) != self_id() {
            result(999);
            return;
        }
        let copied = ExitReason::Kill;
        match copied {
            ExitReason::Normal => {
                result(999);
            }
            ExitReason::Killed => {
                result(999);
            }
            ExitReason::Failure => {
                result(999);
            }
            ExitReason::NoProcess => {
                result(999);
            }
            ExitReason::Kill => {
                result(exit_reason(notification));
            }
        }
        self_exit(copied);
        mark(999);
    }
}
fn main() {
    let rt = actual_runtime::Runtime::new(1);
    let h = rt.spawn(checked::program(), actual_std::vec![42]).unwrap();
    let deadline = actual_std::time::Instant::now() + actual_std::time::Duration::from_secs(30);
    loop {
        let s = h.snapshot();
        if s.cleanup_event > 0 {
            assert_eq!(
                (
                    s.terminal,
                    s.exit_reason,
                    s.stage,
                    s.result,
                    s.receives,
                    s.drops,
                    s.freed_bytes
                ),
                (3, 5, 42, 1, 2, 1, 8)
            );
            break;
        }
        assert!(
            actual_std::time::Instant::now() < deadline,
            "correctness watchdog"
        );
        actual_std::thread::yield_now();
    }
    rt.shutdown();
    actual_std::println!("SHADOW_EXIT_PASSED");
}
