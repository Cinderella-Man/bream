use bream_link_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
