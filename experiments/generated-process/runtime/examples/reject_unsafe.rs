use bream_probe_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
