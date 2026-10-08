use bream_replacement_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
