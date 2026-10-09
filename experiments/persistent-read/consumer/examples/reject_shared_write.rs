use bream_stream_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let mut bytes = shared_bytes(receive_shared_any());
        write(bytes, 0, 42);
    }
}
fn main() {}
