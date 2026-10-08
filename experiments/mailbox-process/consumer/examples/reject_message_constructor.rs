use bream_mailbox_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let message = Message {
            tag: 1,
            words: owned_input(),
        };
        result(message_tag(message));
    }
}
fn main() {}
