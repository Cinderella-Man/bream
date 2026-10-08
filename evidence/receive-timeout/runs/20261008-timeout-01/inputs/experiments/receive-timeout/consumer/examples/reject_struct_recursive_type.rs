use bream_timeout_macros::process_module;
#[process_module]
mod checked {
    struct State {
        next: State,
    }
    fn run() {}
}
fn main() {}
