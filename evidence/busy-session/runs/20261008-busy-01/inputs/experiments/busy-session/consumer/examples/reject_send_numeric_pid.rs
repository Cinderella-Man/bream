use bream_busy_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        send(1, 7, owned_input());
    }
}
fn main() {}
