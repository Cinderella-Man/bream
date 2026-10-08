use bream_monitor_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        arbitrary().await;
    }
}
fn main() {}
