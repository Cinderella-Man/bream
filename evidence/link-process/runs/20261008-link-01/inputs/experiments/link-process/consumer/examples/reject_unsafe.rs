use bream_link_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
