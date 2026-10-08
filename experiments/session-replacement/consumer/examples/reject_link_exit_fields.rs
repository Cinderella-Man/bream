use bream_replacement_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let exit = receive_exit(self_id());
        result(exit.reason);
    }
}
fn main() {}
