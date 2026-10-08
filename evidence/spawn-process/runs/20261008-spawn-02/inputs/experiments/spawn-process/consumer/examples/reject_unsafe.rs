use bream_spawn_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
