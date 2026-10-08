use bream_link_macros::process;

#[process]
fn forbidden() {
    std::thread::scope(|_| {});
}

fn main() {}
