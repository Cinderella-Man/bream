use bream_spawn_macros::process_module;
#[process_module]
mod rejected {
    fn run() { spawn_monitor(|| {}, owned_input(), 0, 0); }
}
fn main() {}
