use bream_milestone_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
