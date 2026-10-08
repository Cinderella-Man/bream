use bream_shared_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
