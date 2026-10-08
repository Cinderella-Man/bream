use bream_identity_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
