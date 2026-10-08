use bream_timeout_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let message = receive_any();
        let first = message_words(message);
        let second = message_words(message);
        result(len(first) + len(second));
    }
}
fn main() {}
