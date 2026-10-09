use bream_exit_macros::process_module;
#[process_module]
mod rejected { fn run() { self_exit(ExitReason::Normal, ExitReason::Kill); } }
fn main() {}
