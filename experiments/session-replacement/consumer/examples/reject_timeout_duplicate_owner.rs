use bream_replacement_macros::process_module;
#[process_module]
mod rejected {
    fn consume(selected: ReceiveResult) {
        match selected {
            ReceiveResult::Message(message) => {
                result(message_tag(message));
            }
            ReceiveResult::Timeout => {
                result(0);
            }
        }
    }
    fn run() {
        let selected = receive_for(7, 5);
        consume(selected);
        consume(selected);
    }
}
fn main() {}
