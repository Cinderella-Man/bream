use bream_mailbox_macros::process_module;
#[process_module]
mod checked {
    struct State {
        words: Words,
        value: u64,
    }
    fn consume(state: State) -> State {
        return state;
    }
    fn run() {
        let state = State {
            words: owned_input(),
            value: 1,
        };
        let moved = consume(state);
        result(state.value);
    }
}
fn main() {}
