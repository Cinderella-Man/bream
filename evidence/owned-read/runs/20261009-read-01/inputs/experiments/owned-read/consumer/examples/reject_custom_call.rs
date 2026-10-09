use bream_read_macros::process;

#[process]
fn forbidden() {
    std::thread::scope(|_| {});
}

fn main() {}
