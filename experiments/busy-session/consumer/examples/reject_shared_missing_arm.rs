use bream_busy_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        match receive_shared_any_for(0) {
            SharedReceiveResult::Message(message) => {
                result(shared_tag(message));
            }
        }
    }
}
fn main() {}
