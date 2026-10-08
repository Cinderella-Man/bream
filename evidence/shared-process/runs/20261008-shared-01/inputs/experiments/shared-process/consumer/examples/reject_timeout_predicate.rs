use bream_shared_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let selected = receive_for(|item| true, 5);
    }
}
fn main() {}
