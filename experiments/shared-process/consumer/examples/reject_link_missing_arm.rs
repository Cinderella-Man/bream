use bream_shared_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        match receive_event() {
            Event::Message(message) => {
                result(message_tag(message));
            }
            Event::Shared(shared_message) => {
                result(shared_tag(shared_message));
            }
            Event::Down(down) => {
                result(down_reason(down));
            }
        }
    }
}
fn main() {}
