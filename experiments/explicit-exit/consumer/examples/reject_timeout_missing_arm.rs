use bream_exit_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        match receive_for(7, 5) {
            ReceiveResult::Timeout => {}
        }
    }
}
fn main() {}
