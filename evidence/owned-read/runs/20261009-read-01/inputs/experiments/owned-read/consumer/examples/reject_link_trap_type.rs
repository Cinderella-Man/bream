use bream_read_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        trap_exit(1);
    }
}
fn main() {}
