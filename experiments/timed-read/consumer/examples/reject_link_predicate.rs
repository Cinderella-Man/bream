use bream_timed_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let exit = receive_exit(|candidate| true);
    }
}
fn main() {}
