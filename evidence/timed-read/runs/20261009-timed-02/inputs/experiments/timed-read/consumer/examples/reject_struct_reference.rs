use bream_timed_macros::process_module;
#[process_module]
mod checked {
    fn use_ref(value: &Words) -> u64 {
        return len(value);
    }
    fn run() {}
}
fn main() {}
