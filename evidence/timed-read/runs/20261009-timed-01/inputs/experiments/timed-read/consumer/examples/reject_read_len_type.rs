use bream_timed_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        result(read_len(owned_input()));
    }
}
fn main() {}
