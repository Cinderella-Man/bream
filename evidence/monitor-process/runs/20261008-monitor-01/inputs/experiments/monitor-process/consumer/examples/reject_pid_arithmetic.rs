use bream_monitor_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let bad = self_id() + 1;
        send(bad, 7, owned_input());
    }
}
fn main() {}
