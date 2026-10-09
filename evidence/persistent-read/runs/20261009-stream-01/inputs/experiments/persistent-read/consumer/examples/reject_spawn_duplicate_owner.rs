use bream_stream_macros::process_module;
#[process_module]
mod rejected {
    fn child() {}
    fn run() {
        let words = owned_input();
        spawn_monitor(child, words, 0, 0);
        spawn_link(child, words, 0, 0);
    }
}
fn main() {}
