use bream_link_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        println!("escape");
    }
}
fn main() {}
