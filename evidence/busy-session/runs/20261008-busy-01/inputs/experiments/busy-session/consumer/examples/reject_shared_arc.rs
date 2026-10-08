use bream_busy_macros::process_module;
#[process_module]
mod rejected {
    struct State {
        bytes: Arc<SharedBytes>,
    }
    fn run() {}
}
fn main() {}
