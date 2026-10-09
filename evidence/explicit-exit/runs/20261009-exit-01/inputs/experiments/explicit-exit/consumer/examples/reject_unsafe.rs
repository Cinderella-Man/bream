use bream_exit_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
