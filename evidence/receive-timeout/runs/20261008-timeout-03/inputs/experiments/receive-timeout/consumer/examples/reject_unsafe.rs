use bream_timeout_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
