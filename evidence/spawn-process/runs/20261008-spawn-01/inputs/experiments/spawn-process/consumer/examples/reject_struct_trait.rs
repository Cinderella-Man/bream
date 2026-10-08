use bream_spawn_macros::process_module;
#[process_module]
mod checked {
    trait Evil {
        fn escape();
    }
    fn run() {}
}
fn main() {}
