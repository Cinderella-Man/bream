use bream_exit_macros::process_module;
#[process_module]
mod rejected { fn self_exit() {} fn run() { self_exit(); } }
fn main() {}
