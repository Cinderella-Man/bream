use bream_spawn_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let value = ExitResult::Exit(receive_exit(self_id()));
    }
}
fn main() {}
