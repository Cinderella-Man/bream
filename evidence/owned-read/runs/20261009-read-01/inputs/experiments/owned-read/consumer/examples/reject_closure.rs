use bream_read_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
