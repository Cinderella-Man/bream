use bream_exit_macros::process_module;
#[process_module]
mod checked {
    enum Action {
        Update(Words),
    }
    fn run() {}
}
fn main() {}
