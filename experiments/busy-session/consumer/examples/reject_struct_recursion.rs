use bream_busy_macros::process_module;
#[process_module]
mod checked {
    fn recursive(value: u64) -> u64 {
        return recursive(value);
    }
    fn run() {
        result(recursive(1));
    }
}
fn main() {}
