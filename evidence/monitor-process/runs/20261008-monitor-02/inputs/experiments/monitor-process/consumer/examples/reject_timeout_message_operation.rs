use bream_monitor_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let data = message_words(receive_for(7, 5));
    }
}
fn main() {}
