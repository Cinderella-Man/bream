use bream_mailbox_runtime::ProcessId;
fn main() {
    let _ = ProcessId {
        runtime: 1,
        slot: 0,
        generation: 1,
    };
}
