use bream_exit_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let down = receive_down(|value| value == 1);
    }
}
fn main() {}
