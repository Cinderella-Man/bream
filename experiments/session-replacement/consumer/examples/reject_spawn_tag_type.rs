use bream_replacement_macros::process_module;
#[process_module]
mod rejected {
    fn child() {}
    fn run() {
        spawn_monitor(child, owned_input(), false, 0);
    }
}
fn main() {}
