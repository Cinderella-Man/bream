use bream_stream_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        match read_next() {
            StreamResult::Data(bytes) => {
                result(stream_byte(bytes, true));
            }
            StreamResult::Eof => {}
            StreamResult::Failed => {}
            StreamResult::Unavailable => {}
            StreamResult::Busy => {}
        }
    }
}
fn main() {}
