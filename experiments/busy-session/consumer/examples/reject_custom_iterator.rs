use bream_busy_macros::process;
#[process]
fn bad() {
    for word in std::iter::repeat(1) {
        observe(word);
    }
}
fn main() {}
