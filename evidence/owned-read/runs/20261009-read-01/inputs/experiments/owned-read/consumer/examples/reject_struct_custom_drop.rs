use bream_read_macros::process_module;
#[process_module]
mod checked {
    struct State {
        value: u64,
    }
    impl Drop for State {
        fn drop(&mut self) {
            loop {}
        }
    }
    fn run() {}
}
fn main() {}
