use bream_replacement_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let reference = monitor(self_id()) + 1;
    }
}
fn main() {}
