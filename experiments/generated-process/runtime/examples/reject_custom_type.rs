use bream_probe_macros::process;

#[process]
fn forbidden() {
    let custom: String = 0;
}

fn main() {}
