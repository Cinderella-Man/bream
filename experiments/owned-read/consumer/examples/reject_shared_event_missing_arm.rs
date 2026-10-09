use bream_read_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        match receive_event() {
            Event::Message(message) => {
                result(message_tag(message));
            }
            Event::Down(down) => {
                result(down_reason(down));
            }
            Event::Exit(exit) => {
                result(exit_reason(exit));
            }
        }
    }
}
fn main() {}
