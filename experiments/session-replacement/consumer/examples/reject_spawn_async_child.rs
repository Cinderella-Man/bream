use bream_replacement_macros::process_module;
#[process_module]
mod rejected {
    async fn child() {}
    fn run() {
        spawn_monitor(child, owned_input(), 0, 0);
    }
}
fn main() {}
