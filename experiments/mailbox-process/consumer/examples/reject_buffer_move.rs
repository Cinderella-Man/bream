use bream_mailbox_macros::process;
#[process]
fn bad() {
    let words = owned_input();
    let moved = words;
}
fn main() {}
