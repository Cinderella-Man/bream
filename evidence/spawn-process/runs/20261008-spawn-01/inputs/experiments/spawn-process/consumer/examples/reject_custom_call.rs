use bream_spawn_macros::process;

#[process]
fn forbidden() {
    std::thread::scope(|_| {});
}

fn main() {}
