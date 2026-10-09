use bream_stream_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        match read_input() {
            ReadResult::Other => {}
        }
    }
}
fn main() {}
