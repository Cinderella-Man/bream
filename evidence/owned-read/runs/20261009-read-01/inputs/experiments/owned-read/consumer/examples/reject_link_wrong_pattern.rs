use bream_read_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        match receive_exit_for(self_id(), 0) {
            DownResult::Down(down) => {
                result(down_reason(down));
            }
            DownResult::Timeout => {
                result(0);
            }
        }
    }
}
fn main() {}
