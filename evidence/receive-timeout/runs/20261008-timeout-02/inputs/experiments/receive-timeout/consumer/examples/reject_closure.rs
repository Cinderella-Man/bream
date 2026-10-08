use bream_timeout_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
