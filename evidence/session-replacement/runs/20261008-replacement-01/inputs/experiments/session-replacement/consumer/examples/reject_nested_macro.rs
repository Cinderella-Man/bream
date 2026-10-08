use bream_replacement_macros::process;
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
