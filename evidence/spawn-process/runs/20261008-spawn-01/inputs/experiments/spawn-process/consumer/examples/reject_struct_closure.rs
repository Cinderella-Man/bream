use bream_spawn_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        let callback = || loop {};
    }
}
fn main() {}
