use bream_read_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let message = receive_shared(|tag| tag == 1);
    }
}
fn main() {}
