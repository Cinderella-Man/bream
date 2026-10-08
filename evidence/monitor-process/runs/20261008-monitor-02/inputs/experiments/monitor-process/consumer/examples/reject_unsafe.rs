use bream_monitor_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
