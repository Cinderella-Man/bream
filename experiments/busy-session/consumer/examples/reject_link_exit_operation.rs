use bream_busy_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        result(exit_reason(receive_exit_for(self_id(), 0)));
    }
}
fn main() {}
