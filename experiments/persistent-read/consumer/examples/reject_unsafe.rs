use bream_stream_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
