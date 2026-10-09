use bream_timed_macros::process;

#[process]
fn forbidden() {
    std::thread::scope(|_| {});
}

fn main() {}
