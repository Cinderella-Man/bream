use bream_exit_macros::process_module;
#[process_module]
mod checked {
    struct State {
        value: std::string::String,
    }
    fn run() {}
}
fn main() {}
