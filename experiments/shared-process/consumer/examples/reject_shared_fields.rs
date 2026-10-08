use bream_shared_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let bytes = shared_bytes(receive_shared_any());
        result(bytes.inner);
    }
}
fn main() {}
