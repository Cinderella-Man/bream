use bream_link_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        advance_to(5);
    }
}
fn main() {}
