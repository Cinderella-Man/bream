use bream_timed_macros::process_module;
#[process_module]
mod rejected { fn run() { let reason = ExitReason::Normal; match reason { ExitReason::Normal => {} } } }
fn main() {}
