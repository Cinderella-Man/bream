use bream_exit_macros::process_module;
const EXTERNAL: u64 = 9;
#[process_module]
mod checked {
    fn run() {
        result(EXTERNAL);
    }
}
fn main() {}
