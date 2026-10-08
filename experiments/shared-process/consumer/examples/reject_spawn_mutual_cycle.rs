use bream_shared_macros::process_module;
#[process_module]
mod rejected {
    fn child() {
        spawn_monitor(run, owned_input(), 0, 0);
    }
    fn run() {
        spawn_monitor(child, owned_input(), 0, 0);
    }
}
fn main() {}
