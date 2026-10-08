use bream_mailbox_macros::process_module;
#[process_module]
mod checked {
    enum Action {
        First,
        Second,
    }
    fn run() {
        let action = Action::First;
        match action {
            Action::First => {}
        }
    }
}
fn main() {}
