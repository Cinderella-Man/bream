use bream_stream_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let bytes = SharedBytes::new(owned_input());
    }
}
fn main() {}
