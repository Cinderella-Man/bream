use bream_read_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let bytes: Words = shared_bytes(receive_shared_any());
    }
}
fn main() {}
