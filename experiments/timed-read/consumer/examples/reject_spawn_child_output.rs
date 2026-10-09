use bream_timed_macros::process_module;
#[process_module]
mod rejected {
    fn child() -> u64 {
        return 1;
    }
    fn run() {
        spawn_monitor(child, owned_input(), 0, 0);
    }
}
fn main() {}
