use bream_structured_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        unsafe {}
    }
}
fn main() {}
