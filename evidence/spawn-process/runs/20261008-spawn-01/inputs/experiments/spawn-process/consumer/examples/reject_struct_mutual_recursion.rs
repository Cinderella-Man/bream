use bream_spawn_macros::process_module;
#[process_module]
mod checked {
    fn first(value: u64) -> u64 {
        return second(value);
    }
    fn second(value: u64) -> u64 {
        return first(value);
    }
    fn run() {}
}
fn main() {}
