use bream_exit_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
