use bream_stream_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        match read_input() {
            ReadResult::Data(bytes) => {
                let copied = clone_shared(bytes);
            }
            ReadResult::Eof => {}
            ReadResult::Failed => {}
            ReadResult::Unavailable => {}
        }
    }
}
fn main() {}
