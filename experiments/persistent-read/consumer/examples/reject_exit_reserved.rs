use bream_stream_macros::process_module;
#[process_module]
mod rejected { fn run() { let ExitReason = 1; result(ExitReason); } }
fn main() {}
