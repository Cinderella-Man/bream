use bream_replacement_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
