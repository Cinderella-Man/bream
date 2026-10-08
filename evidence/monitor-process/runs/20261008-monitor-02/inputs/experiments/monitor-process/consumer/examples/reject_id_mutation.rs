use bream_monitor_runtime::ProcessId;
fn change(mut id: ProcessId) {
    id.generation = 0;
}
fn main() {}
