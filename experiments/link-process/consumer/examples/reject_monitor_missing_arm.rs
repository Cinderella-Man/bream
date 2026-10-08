use bream_link_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        match receive_event() {
            Event::Down(down) => {
                result(down_reason(down));
            }
        }
    }
}
fn main() {}
