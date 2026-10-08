use bream_replacement_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let selected = ReceiveResult::Timeout;
    }
}
fn main() {}
