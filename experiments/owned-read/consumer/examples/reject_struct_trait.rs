use bream_read_macros::process_module;
#[process_module]
mod checked {
    trait Evil {
        fn escape();
    }
    fn run() {}
}
fn main() {}
