use bream_monitor_macros::process_module;
#[process_module]
mod rejected {
    fn consume(selected: Event) {
        match selected {
            Event::Message(message) => {
                result(message_tag(message));
            }
            Event::Down(down) => {
                result(down_reason(down));
            }
        }
    }
    fn run() {
        let event = receive_event();
        consume(event);
        consume(event);
    }
}
fn main() {}
