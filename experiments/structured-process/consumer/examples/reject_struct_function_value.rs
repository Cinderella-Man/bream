use bream_structured_macros::process_module;
#[process_module]
mod checked {
    fn helper(value: u64) -> u64 {
        return value;
    }
    fn run() {
        let callback = helper;
    }
}
fn main() {}
