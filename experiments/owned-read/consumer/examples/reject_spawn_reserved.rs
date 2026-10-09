use bream_read_macros::process_module;
#[process_module]
mod rejected {
    struct Spawned {
        value: u64,
    }
    fn run() {}
}
fn main() {}
