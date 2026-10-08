use bream_busy_macros::process_module;
#[process_module]
mod rejected {
    fn child() {}
    fn run() {
        let created = spawn_monitor(child, owned_input(), 0, 0);
        let identity = created.pid;
    }
}
fn main() {}
