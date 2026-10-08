use bream_link_macros::process_module;
#[process_module]
mod checked {
    struct State {
        next: State,
    }
    fn run() {}
}
fn main() {}
