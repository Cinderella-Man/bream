use bream_timed_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
