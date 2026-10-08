use bream_probe_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
