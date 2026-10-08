use bream_link_macros::process;
#[process]
fn bad() {
    let word: u64 = 1;
    result(read(word, 0));
}
fn main() {}
