use bream_timed_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        spawn_monitor(outside::child, owned_input(), 0, 0);
    }
}
fn main() {}
