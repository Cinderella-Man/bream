use bream_exit_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        unlink(monitor(self_id()));
    }
}
fn main() {}
