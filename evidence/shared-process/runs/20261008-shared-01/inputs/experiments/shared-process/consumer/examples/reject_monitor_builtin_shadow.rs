use bream_shared_macros::process_module;
#[process_module]
mod rejected {
    fn demonitor_flush(value: u64) {
        result(value);
    }
    fn run() {
        demonitor_flush(7);
    }
}
fn main() {}
