use bream_exit_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        match receive_event() {
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
