use bream_shared_macros::process_module;
#[process_module]
mod checked {
    pub struct State {
        value: u64,
    }
    fn run() {}
}
fn main() {}
