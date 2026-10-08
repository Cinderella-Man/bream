use bream_spawn_macros::process_module;
#[process_module]
mod checked {
    struct State {
        r#__bream_context: u64,
    }
    fn run() {}
}
fn main() {}
