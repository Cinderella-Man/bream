use bream_exit_macros::process;

#[process]
fn forbidden() {
    std::thread::scope(|_| {});
}

fn main() {}
