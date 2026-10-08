use bream_monitor_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
