use bream_timed_macros::process;
#[process]
fn bad() {
    for word in std::iter::repeat(1) {
        observe(word);
    }
}
fn main() {}
