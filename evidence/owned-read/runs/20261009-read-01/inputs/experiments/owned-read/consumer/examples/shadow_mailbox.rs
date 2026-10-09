extern crate bream_read_runtime as actual_runtime;
extern crate self as bream_read_runtime;
extern crate self as core;
extern crate self as std;
extern crate std as actual_std;
pub struct ProcessId;
pub struct Envelope;
pub mod option {
    pub struct Option;
}
pub mod primitive {
    #[allow(non_camel_case_types)]
    pub type u64 = u8;
}
use bream_read_macros::process_module;
#[process_module]
mod checked {
    fn target() -> Pid {
        return self_id();
    }
    fn run() {
        send(target(), 1000, owned_input());
        let message = receive(1000);
        let tag = message_tag(message);
        let words = message_words(message);
        result(tag + read(words, 0));
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
                (s.terminal, s.result, s.drops, s.freed_bytes),
                (1, 1042, 1, 8)
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
    actual_std::println!("SHADOW_MAILBOX_PASSED");
}
