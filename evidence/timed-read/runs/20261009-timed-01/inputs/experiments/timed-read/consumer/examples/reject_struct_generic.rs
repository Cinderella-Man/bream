use bream_timed_macros::process_module;
#[process_module]
mod checked {
    fn generic<T>(value: T) -> u64 {
        return 1;
    }
    fn run() {}
}
fn main() {}
