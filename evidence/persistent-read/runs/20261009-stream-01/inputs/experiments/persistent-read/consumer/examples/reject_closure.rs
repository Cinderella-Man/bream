use bream_stream_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
