use bream_read_macros::process;

#[process]
fn forbidden() {
    observe(1).await;
}

fn main() {}
