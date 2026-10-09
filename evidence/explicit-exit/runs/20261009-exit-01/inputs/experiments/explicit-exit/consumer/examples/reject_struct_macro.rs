use bream_exit_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        println!("escape");
    }
}
fn main() {}
