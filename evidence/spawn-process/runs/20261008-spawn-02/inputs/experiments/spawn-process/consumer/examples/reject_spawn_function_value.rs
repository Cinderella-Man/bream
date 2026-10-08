use bream_spawn_macros::process_module;
#[process_module]
mod rejected {
    fn child() {} fn run() { let factory = child; spawn_monitor(factory, owned_input(), 0, 0); }
}
fn main() {}
