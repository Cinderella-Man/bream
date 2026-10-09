use bream_read_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        link(7);
    }
}
fn main() {}
