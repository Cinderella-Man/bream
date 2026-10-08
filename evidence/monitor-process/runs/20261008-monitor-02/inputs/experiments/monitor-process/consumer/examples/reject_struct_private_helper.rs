use bream_monitor_macros::process_module;
#[process_module]
mod checked {
    fn helper(value: u64) -> u64 {
        return value;
    }
    fn run() {}
}
fn main() {
    let _ = checked::__bream_helper_helper;
}
