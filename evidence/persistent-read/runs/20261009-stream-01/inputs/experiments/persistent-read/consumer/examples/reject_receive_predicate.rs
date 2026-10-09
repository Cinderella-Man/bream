use bream_stream_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let message = receive(|value| value == 1);
        result(message_tag(message));
    }
}
fn main() {}
