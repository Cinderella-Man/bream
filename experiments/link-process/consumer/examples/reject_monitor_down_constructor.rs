use bream_link_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let down = Down { reason: 1 };
    }
}
fn main() {}
