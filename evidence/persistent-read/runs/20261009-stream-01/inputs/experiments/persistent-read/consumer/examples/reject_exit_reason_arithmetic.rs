use bream_stream_macros::process_module;
#[process_module]
mod rejected { fn run() { let value = ExitReason::Normal + 1; result(value); } }
fn main() {}
