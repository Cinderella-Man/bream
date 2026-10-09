use bream_read_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let bytes = shared_bytes(receive_shared_any());
        send_shared(self_id(), 1, bytes);
        send_shared(self_id(), 2, bytes);
    }
}
fn main() {}
