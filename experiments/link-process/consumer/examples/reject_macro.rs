use bream_link_macros::process;

#[process]
fn forbidden() {
    println!("not an approved builtin");
}

fn main() {}
