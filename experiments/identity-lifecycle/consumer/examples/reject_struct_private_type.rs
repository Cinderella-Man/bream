use bream_identity_macros::process_module;
#[process_module]
mod checked {
    struct State {
        value: u64,
    }
    fn run() {}
}
impl Drop for checked::State {
    fn drop(&mut self) {
        loop {}
    }
}
fn main() {}
