use bream_shared_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let factory: Program = program();
        spawn_monitor(factory, owned_input(), 0, 0);
    }
}
fn main() {}
