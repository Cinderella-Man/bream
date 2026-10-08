use bream_replacement_macros::process;

#[process]
fn forbidden() {
    observe(1).await;
}

fn main() {}
