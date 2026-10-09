use bream_timed_macros::process_module;
#[process_module]
mod checked {
    fn run() {
        let value: StreamResult = read_next_for(5);
    }
}
fn main() {}
