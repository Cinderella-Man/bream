use bream_shared_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        fn nested() {
            loop {}
        }
    }
}
fn main() {}
