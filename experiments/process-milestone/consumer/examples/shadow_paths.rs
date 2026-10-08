extern crate bream_milestone_runtime as actual_runtime;
extern crate std as actual_std;
extern crate self as bream_milestone_runtime;
extern crate self as core;
extern crate self as std;

// These names must never alter the expansion's types, factory or checkpoint.
pub mod primitive { #[allow(non_camel_case_types)] pub type u64 = u8; }
pub struct Program;
pub struct ProcessContext;
pub struct OwnedWords;
pub fn checkpoint() { loop {} }

use bream_milestone_macros::process;
#[process]
fn verified() {
    let mut words = owned_input();
    let mut total: u64 = 1000;
    for index in 0..len(words) {
        total += read(words, index);
        write(words, index, total);
    }
    result(total);
}

fn main() {
    let runtime = actual_runtime::Runtime::new(1);
    let handle = runtime.spawn(verified(), actual_std::vec![3, 5, 7]).unwrap();
    let start = actual_std::time::Instant::now();
    loop {
        let s = handle.snapshot();
        if s.cleanup_ns != 0 {
            assert_eq!(s.terminal, 1);
            assert_eq!(s.result, 1015);
            assert!(s.polls > 1);
            break;
        }
        assert!(start.elapsed().as_secs() < 5);
        actual_std::thread::yield_now();
    }
    runtime.shutdown();
    actual_std::println!("SHADOW_PATHS_PASSED");
}
