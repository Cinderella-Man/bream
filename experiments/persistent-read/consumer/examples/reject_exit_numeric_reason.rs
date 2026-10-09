use bream_stream_macros::process_module;
#[process_module]
mod rejected { fn run() { self_exit(3); } }
fn main() {}
