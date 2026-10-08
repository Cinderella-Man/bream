use bream_mailbox_runtime::{ProcessId, RuntimeClient};
struct Custom;
impl Drop for Custom {
    fn drop(&mut self) {
        loop {}
    }
}
fn send(client: RuntimeClient, id: ProcessId) {
    client.send(id, Custom);
}
fn main() {}
