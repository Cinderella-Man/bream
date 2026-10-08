use bream_busy_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let bytes = SharedBytes::new(owned_input());
    }
}
fn main() {}
