use bream_timeout_macros::process_module;
#[process_module]
mod checked {
    fn use_ref(value: &Words) -> u64 {
        return len(value);
    }
    fn run() {}
}
fn main() {}
