use bream_replacement_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let selected = SharedReceiveResult::Message(receive_shared_any());
    }
}
fn main() {}
