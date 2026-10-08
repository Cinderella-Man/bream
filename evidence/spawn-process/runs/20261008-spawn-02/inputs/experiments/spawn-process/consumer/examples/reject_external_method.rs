use bream_spawn_macros::process;
#[process]
fn bad() {
    let word: u64 = 1;
    word.to_string();
}
fn main() {}
