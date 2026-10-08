use bream_shared_macros::process;
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
