use bream_read_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
