use bream_stream_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let bytes = clone_shared(shared_bytes(receive_shared_any()));
    }
}
fn main() {}
