use bream_timeout_macros::process_module;
#[process_module]
mod rejected {
    fn message_words(value: u64) -> u64 {
        return value;
    }
    fn run() {
        result(message_words(7));
    }
}
fn main() {}
