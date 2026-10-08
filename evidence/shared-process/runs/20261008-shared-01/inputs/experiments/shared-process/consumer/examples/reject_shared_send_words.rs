use bream_shared_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        send_shared(self_id(), 1, owned_input());
    }
}
fn main() {}
