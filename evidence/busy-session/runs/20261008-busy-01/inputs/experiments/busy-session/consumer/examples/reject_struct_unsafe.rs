use bream_busy_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        unsafe {}
    }
}
fn main() {}
