use bream_timed_macros::process_module;
#[process_module]
mod checked {
    enum Action {
        Update(u64),
    }
    fn run() {
        let action = Action::Update(true);
    }
}
fn main() {}
