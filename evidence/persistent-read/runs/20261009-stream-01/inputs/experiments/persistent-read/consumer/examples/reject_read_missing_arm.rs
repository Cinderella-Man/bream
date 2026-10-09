use bream_stream_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        match read_input() {
            ReadResult::Data(bytes) => {
                result(read_len(bytes));
            }
            ReadResult::Eof => {}
            ReadResult::Failed => {}
        }
    }
}
fn main() {}
