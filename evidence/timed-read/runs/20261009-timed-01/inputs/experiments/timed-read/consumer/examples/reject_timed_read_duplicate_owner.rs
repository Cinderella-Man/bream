use bream_timed_macros::process_module;
#[process_module]
mod checked {
    fn take(value: TimedStreamResult) {}
    fn run() {
        let value = read_next_for(5);
        take(value);
        take(value);
    }
}
fn main() {}
