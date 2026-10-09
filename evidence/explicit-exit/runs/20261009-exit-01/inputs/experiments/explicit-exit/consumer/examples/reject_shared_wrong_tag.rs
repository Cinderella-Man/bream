use bream_exit_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let message = receive_shared(true);
    }
}
fn main() {}
