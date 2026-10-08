extern crate bream_spawn_runtime as actual_runtime;
extern crate self as bream_spawn_runtime;
extern crate self as core;
extern crate self as std;
extern crate std as actual_std;
pub struct Program;
pub struct Spawned;
pub struct ProcessId;
pub struct ProcessContext;
pub struct OwnedWords;
pub struct MonitorRef;
pub mod boxed {
    pub struct Box;
}
pub mod primitive {
    #[allow(non_camel_case_types)]
    pub type u64 = u8;
    #[allow(non_camel_case_types)]
    pub type bool = u8;
}
use bream_spawn_macros::process_module;
#[process_module]
mod checked {
    struct State {
        child: Spawned,
        words: Words,
    }
    fn words_once() -> Words {
        mark(10);
        return owned_input();
    }
    fn tag_once() -> u64 {
        mark(20);
        return 71;
    }
    fn argument_once() -> u64 {
        mark(30);
        return 5;
    }
    fn child() {
        let words = owned_input();
        let digest = read(words, 0) + input_tag() + input_argument();
        send(input_target(), digest, words);
    }
    fn finish(state: State) {
        let duplicate = state.child;
        let down = receive_down(spawned_ref(duplicate));
        if down_target(down) != spawned_pid(state.child)
            || down_ref(down) != spawned_ref(state.child)
        {
            result(999);
            return;
        }
        result(read(state.words, 0) * 10 + down_reason(down));
    }
    fn run() {
        let created: Spawned = spawn_monitor(child, words_once(), tag_once(), argument_once());
        let message = receive_any();
        if message_tag(message) != 118 {
            result(999);
            return;
        }
        finish(State {
            child: created,
            words: message_words(message),
        });
    }
}
fn main() {
    let rt = actual_runtime::Runtime::new(1);
    let client = rt.client();
    let handle = rt.spawn(checked::program(), actual_std::vec![42]).unwrap();
    let deadline = actual_std::time::Instant::now() + actual_std::time::Duration::from_secs(30);
    loop {
        let snapshot = handle.snapshot();
        if snapshot.cleanup_event > 0 {
            assert_eq!(
                (snapshot.terminal, snapshot.result, snapshot.stage),
                (1, 421, 30)
            );
            assert_eq!(
                (snapshot.drops, snapshot.live_bytes, snapshot.freed_bytes),
                (1, 0, 8)
            );
            assert_eq!((client.active_links(), client.active_monitors()), (0, 0));
            break;
        }
        assert!(
            actual_std::time::Instant::now() < deadline,
            "correctness watchdog"
        );
        actual_std::thread::yield_now();
    }
    rt.shutdown();
    actual_std::println!("SHADOW_SPAWN_PASSED");
}
