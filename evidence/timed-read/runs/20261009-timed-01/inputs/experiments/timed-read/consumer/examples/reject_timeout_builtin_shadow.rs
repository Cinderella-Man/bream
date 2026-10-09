use bream_timed_macros::process_module;
#[process_module]
mod rejected {
    fn receive_for() -> u64 {
        return 5;
    }
    fn run() {}
}
fn main() {}
