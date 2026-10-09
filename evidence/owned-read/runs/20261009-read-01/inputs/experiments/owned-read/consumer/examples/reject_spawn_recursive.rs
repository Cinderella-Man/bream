use bream_read_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        spawn_link(run, owned_input(), 0, 0);
    }
}
fn main() {}
