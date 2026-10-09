use bream_exit_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let reference = monitor(7);
    }
}
fn main() {}
