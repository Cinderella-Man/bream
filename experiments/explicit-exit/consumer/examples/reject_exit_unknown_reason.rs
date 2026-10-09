use bream_exit_macros::process_module;
#[process_module]
mod rejected { fn run() { self_exit(ExitReason::Arbitrary); } }
fn main() {}
