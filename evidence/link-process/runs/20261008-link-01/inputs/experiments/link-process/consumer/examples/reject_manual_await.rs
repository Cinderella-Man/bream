use bream_link_macros::process;

#[process]
fn forbidden() {
    observe(1).await;
}

fn main() {}
