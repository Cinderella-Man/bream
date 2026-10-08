use bream_mailbox_macros::process_module;
#[process_module]
mod checked {
    #[derive(Clone)]
    struct State {
        value: u64,
    }
    fn run() {}
}
fn main() {}
