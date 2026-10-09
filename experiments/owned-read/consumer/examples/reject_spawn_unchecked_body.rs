use bream_read_macros::process_module;
#[process_module]
mod rejected {
    fn child() {
        external_work();
    }
    fn run() {
        spawn_monitor(child, owned_input(), 0, 0);
    }
}
fn main() {}
