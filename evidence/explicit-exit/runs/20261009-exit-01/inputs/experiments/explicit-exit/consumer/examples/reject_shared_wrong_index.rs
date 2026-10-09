use bream_exit_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let bytes = shared_bytes(receive_shared_any());
        result(shared_read(bytes, true));
    }
}
fn main() {}
