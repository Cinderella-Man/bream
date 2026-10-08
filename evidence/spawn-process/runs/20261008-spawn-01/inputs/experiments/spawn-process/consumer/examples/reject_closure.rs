use bream_spawn_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
