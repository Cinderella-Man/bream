use bream_mailbox_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
