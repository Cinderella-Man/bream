use bream_read_macros::process_module;
#[process_module]
mod rejected { fn run() { let value = ExitReason { code: 1 }; self_exit(value); } }
fn main() {}
