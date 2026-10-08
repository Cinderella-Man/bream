extern crate bream_replacement_runtime as actual_runtime;
extern crate self as bream_replacement_runtime;
extern crate self as core;
extern crate self as std;
extern crate std as actual_std;
pub struct ProcessId;
pub struct Envelope;
pub struct Exit;
pub struct ExitResult;
pub struct LinkResult;
pub struct RuntimeExit;
pub mod option {
    pub struct Option;
}
pub mod primitive {
    #[allow(non_camel_case_types)]
    pub type u64 = u8;
    #[allow(non_camel_case_types)]
    pub type bool = u8;
}
use bream_replacement_macros::process_module;
#[process_module]
mod checked {
    fn target_once() -> Pid {
        // An accidental second operand evaluation would consume the input twice.
        send(self_id(), 1001, owned_input());
        return self_id();
    }
    fn selected() -> Pid {
        mark(20);
        return self_id();
    }
    fn delay() -> u64 {
        mark(21);
        return 0;
    }
    fn enabled() -> bool {
        return true;
    }
    fn describe(exit: Exit) -> u64 {
        let copied = exit;
        let source = exit_from(exit);
        unlink(source);
        return exit_reason(copied);
    }
    fn wait_for(source: Pid) -> Exit {
        return receive_exit(source);
    }
    fn run() {
        let initial = trap_exit(enabled());
        if initial {
            result(9000);
            return;
        }
        let previous = trap_exit(false);
        if !previous {
            result(9001);
            return;
        }
        let disabled = trap_exit(true);
        if disabled {
            result(9002);
            return;
        }
        link(target_once());
        link(self_id());
        match try_link(selected()) {
            LinkResult::Linked => {}
            LinkResult::NoProcess => {
                result(9005);
                return;
            }
        }
        unlink(selected());
        match receive_exit_for(selected(), delay()) {
            ExitResult::Exit(unexpected) => {
                result(9003 + describe(unexpected));
                return;
            }
            ExitResult::Timeout => {}
        }
        let was_enabled = trap_exit(false);
        if !was_enabled {
            result(9004);
            return;
        }
        let message = receive_any();
        let tag = message_tag(message);
        let words = message_words(message);
        result(tag + read(words, 0));
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
            assert_eq!((s.receives, s.timeouts, s.stage), (1, 1, 21));
            break;
        }
        assert!(
            actual_std::time::Instant::now() < deadline,
            "correctness watchdog"
        );
        actual_std::thread::yield_now();
    }
    rt.shutdown();
    actual_std::println!("SHADOW_LINK_PASSED");
}
