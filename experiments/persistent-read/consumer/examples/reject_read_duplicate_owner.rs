use bream_stream_macros::process_module;
#[process_module]
mod checked {
    fn take(bytes: ReadBytes) {
        result(read_len(bytes));
    }
    fn run() {
        match read_input() {
            ReadResult::Data(bytes) => {
                take(bytes);
                take(bytes);
            }
            ReadResult::Eof => {}
            ReadResult::Failed => {}
            ReadResult::Unavailable => {}
        }
    }
}
fn main() {}
