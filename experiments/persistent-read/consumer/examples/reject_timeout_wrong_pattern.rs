use bream_stream_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        match receive_for(7, 5) {
            ReceiveResult::Message => {}
            ReceiveResult::Timeout => {}
        }
    }
}
fn main() {}
