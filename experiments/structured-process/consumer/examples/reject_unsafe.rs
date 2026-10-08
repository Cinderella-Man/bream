use bream_structured_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
