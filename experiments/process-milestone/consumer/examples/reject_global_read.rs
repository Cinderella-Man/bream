use bream_milestone_macros::process;
static GLOBAL: u64 = 1;

#[process]
fn forbidden() {
    let value: u64 = GLOBAL;
}

fn main() {}
