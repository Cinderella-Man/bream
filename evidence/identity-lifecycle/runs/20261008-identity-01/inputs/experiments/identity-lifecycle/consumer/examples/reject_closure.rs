use bream_identity_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
