use bream_stream_macros::process_module;
#[process_module]
mod checked {
    fn take(bytes: StreamBytes) {
        result(stream_len(bytes));
    }
    fn run() {
        match read_next() {
            StreamResult::Data(bytes) => {
                take(bytes);
                take(bytes);
            }
            StreamResult::Eof => {}
            StreamResult::Failed => {}
            StreamResult::Unavailable => {}
            StreamResult::Busy => {}
        }
    }
}
fn main() {}
