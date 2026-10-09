use bream_read_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let exit = receive_exit_for(self_id(), false);
    }
}
fn main() {}
