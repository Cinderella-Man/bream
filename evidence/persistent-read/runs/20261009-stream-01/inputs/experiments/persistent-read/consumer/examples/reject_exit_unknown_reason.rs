use bream_stream_macros::process_module;
#[process_module]
mod rejected { fn run() { self_exit(ExitReason::Arbitrary); } }
fn main() {}
