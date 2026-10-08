use bream_structured_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
