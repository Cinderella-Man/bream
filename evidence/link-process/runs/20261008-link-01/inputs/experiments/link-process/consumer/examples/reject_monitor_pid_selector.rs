use bream_link_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let down = receive_down(self_id());
    }
}
fn main() {}
