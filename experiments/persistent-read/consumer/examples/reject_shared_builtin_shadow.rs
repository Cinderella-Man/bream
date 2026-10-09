use bream_stream_macros::process_module;
#[process_module]
mod rejected {
    fn clone_shared() -> u64 {
        return 1;
    }
    fn run() {}
}
fn main() {}
