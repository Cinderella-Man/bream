use bream_shared_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
