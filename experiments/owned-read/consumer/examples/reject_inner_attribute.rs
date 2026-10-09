use bream_read_macros::process;
#[process]
fn bad() {
    #[allow(unused)]
    let word: u64 = 1;
}
fn main() {}
