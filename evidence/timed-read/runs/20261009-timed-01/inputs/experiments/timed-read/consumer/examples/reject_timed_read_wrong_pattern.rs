use bream_timed_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        match read_next_for(5) {
            TimedStreamResult::Data => {}
            TimedStreamResult::Timeout => {}
            TimedStreamResult::Eof => {}
            TimedStreamResult::Failed => {}
            TimedStreamResult::Unavailable => {}
            TimedStreamResult::Busy => {}
        }
    }
}
fn main() {}
