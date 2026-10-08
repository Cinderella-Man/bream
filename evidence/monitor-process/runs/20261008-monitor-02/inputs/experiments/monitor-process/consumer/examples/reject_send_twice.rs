use bream_monitor_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let words = owned_input();
        send(self_id(), 1, words);
        send(self_id(), 2, words);
    }
}
fn main() {}
