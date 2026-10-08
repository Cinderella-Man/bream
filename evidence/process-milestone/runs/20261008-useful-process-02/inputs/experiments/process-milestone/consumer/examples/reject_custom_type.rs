use bream_milestone_macros::process;

#[process]
fn forbidden() {
    let custom: String = 0;
}

fn main() {}
