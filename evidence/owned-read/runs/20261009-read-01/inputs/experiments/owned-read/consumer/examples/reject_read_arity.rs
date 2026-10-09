use bream_read_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        let value = read_input(1);
    }
}
fn main() {}
