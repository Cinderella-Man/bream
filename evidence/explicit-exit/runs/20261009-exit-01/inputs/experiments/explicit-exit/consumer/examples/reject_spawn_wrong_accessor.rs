use bream_exit_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let identity = spawned_pid(self_id());
    }
}
fn main() {}
