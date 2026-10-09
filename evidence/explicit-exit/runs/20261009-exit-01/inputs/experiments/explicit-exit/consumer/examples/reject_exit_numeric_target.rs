use bream_exit_macros::process_module;
#[process_module]
mod rejected { fn run() { exit_to(1, ExitReason::Kill); } }
fn main() {}
