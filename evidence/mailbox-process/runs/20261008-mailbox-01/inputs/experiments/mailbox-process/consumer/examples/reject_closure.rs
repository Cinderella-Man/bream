use bream_mailbox_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
