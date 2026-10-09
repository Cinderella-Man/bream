use bream_stream_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let message = receive_shared_any();
        let first = shared_bytes(message);
        let second = shared_bytes(message);
        result(shared_len(first) + shared_len(second));
    }
}
fn main() {}
