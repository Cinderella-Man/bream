use bream_busy_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        exit_signal(self_id(), 2);
    }
}
fn main() {}
