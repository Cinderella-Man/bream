use bream_identity_macros::process;

#[process]
fn forbidden() {
    observe(1).await;
}

fn main() {}
