use bream_replacement_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        match receive_shared_any_for(0) {
            ReceiveResult::Message(message) => {
                result(message_tag(message));
            }
            ReceiveResult::Timeout => {}
        }
    }
}
fn main() {}
