use bream_shared_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let message = SharedMessage {
            tag: 1,
            bytes: shared_bytes(receive_shared_any()),
        };
    }
}
fn main() {}
