use bream_milestone_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
