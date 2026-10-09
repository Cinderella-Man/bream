use bream_stream_macros::process;
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
