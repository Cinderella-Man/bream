use bream_read_macros::process_module;
#[process_module]
mod rejected {
    fn child(value: u64) {}
    fn run() {
        spawn_link(child, owned_input(), 0, 0);
    }
}
fn main() {}
