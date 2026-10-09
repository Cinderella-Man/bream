use bream_timed_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        match read_input() {
            ReadResult::Data(bytes) => {
                send_shared(self_id(), 7, bytes);
            }
            ReadResult::Eof => {}
            ReadResult::Failed => {}
            ReadResult::Unavailable => {}
        }
    }
}
fn main() {}
