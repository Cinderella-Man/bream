use bream_shared_macros::process;

#[process]
fn forbidden() {
    let __bream_context: u64 = 0;
}

fn main() {}
