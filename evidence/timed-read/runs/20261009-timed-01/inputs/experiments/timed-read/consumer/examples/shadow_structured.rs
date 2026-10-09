extern crate bream_timed_runtime as actual_runtime;
extern crate self as bream_timed_runtime;
extern crate self as core;
extern crate self as std;
extern crate std as actual_std;
pub mod primitive {
    #[allow(non_camel_case_types)]
    pub type u64 = u8;
}
pub struct Program;
pub struct ProcessContext;
pub struct OwnedWords;
pub fn checkpoint() {
    loop {}
}

use bream_timed_macros::process_module;
#[process_module]
mod checked {
    struct State {
        words: Words,
        total: u64,
    }
    enum Action {
        Add(u64),
        Keep,
    }
    fn update(mut state: State, action: Action) -> State {
        match action {
            Action::Add(amount) => {
                state.total += amount + len(state.words);
            }
            Action::Keep => {}
        }
        return state;
    }
    fn run() {
        let mut state = State {
            words: owned_input(),
            total: 1000,
        };
        state = update(state, Action::Add(15));
        state = update(state, Action::Keep);
        result(state.total);
    }
}
fn main() {
    let runtime = actual_runtime::Runtime::new(1);
    let h = runtime
        .spawn(checked::program(), actual_std::vec![1, 2, 3])
        .unwrap();
    let start = actual_std::time::Instant::now();
    loop {
        let s = h.snapshot();
        if s.cleanup_event > 0 {
            assert_eq!(
                (s.terminal, s.result, s.drops, s.freed_bytes),
                (1, 1018, 1, 24)
            );
            assert!(s.polls > 1);
            break;
        }
        assert!(start.elapsed().as_secs() < 5);
        actual_std::thread::yield_now();
    }
    runtime.shutdown();
    actual_std::println!("SHADOW_STRUCTURED_PASSED");
}
