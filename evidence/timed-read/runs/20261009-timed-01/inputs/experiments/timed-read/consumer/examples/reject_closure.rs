use bream_timed_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
