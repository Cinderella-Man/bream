use bream_shared_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let selected = receive_down_for(monitor(self_id()), true);
    }
}
fn main() {}
