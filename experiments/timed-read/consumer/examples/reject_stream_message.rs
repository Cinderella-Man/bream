use bream_timed_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        match read_next() {
            StreamResult::Data(bytes) => {
                send_shared(self_id(), 7, bytes);
            }
            StreamResult::Eof => {}
            StreamResult::Failed => {}
            StreamResult::Unavailable => {}
            StreamResult::Busy => {}
        }
    }
}
fn main() {}
