use bream_timeout_macros::process_module;
#[process_module]
mod checked {
    struct State {
        value: u64,
    }
    fn run() {
        let state = State { value: true };
    }
}
fn main() {}
