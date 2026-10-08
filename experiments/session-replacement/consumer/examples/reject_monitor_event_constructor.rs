use bream_replacement_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let event = Event::Down(receive_down(monitor(self_id())));
    }
}
fn main() {}
