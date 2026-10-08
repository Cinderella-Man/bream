use bream_busy_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
