use bream_monitor_macros::process_module;
#[process_module]
mod checked {
    struct State {
        value: std::string::String,
    }
    fn run() {}
}
fn main() {}
