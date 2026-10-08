use bream_busy_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
