use bream_timed_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        match read_next_for(5) {
            TimedStreamResult::Cancelled => {}
        }
    }
}
fn main() {}
