use bream_link_macros::process_module;
#[process_module]
mod checked {
    use std::thread;
    fn run() {}
}
fn main() {}
