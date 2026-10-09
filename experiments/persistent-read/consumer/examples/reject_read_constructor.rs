use bream_stream_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        let fake = ReadResult::Data(owned_input());
    }
}
fn main() {}
