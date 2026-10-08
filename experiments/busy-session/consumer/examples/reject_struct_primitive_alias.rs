use bream_busy_macros::process_module;
#[process_module]
mod checked {
    type Number = u64;
    fn run() {}
}
fn main() {}
