use bream_read_macros::process_module;
#[process_module]
mod rejected { fn run() { exit_to(self_id()); } }
fn main() {}
