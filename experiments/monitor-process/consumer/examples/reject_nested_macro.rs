use bream_monitor_macros::process;
macro_rules! hidden {
    () => {
        loop {}
    };
}
#[process]
fn bad() {
    hidden!();
}
fn main() {}
