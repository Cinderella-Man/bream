use bream_read_macros::process_module;
#[process_module]
mod rejected { fn run() { let value = self_exit(ExitReason::Normal); } }
fn main() {}
