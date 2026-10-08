use bream_structured_macros::process_module;
#[process_module]
mod checked {
    fn both(first: Words, second: Words) -> u64 {
        return len(first) + len(second);
    }
    fn run() {
        let words = owned_input();
        result(both(words, words));
    }
}
fn main() {}
