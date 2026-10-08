use bream_shared_macros::process;
#[process]
fn bad() {
    let mut words = owned_input();
    words = 0;
}
fn main() {}
