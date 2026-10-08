use bream_shared_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        result(shared_tag(receive_any()));
    }
}
fn main() {}
