use bream_busy_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let message = receive_shared(true);
    }
}
fn main() {}
