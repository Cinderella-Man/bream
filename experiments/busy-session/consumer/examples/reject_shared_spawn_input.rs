use bream_busy_macros::process_module;
#[process_module]
mod rejected {
    fn child() {}
    fn run() {
        let bytes = shared_bytes(receive_shared_any());
        let child_id = spawn_link(child, bytes, 0, 0);
    }
}
fn main() {}
