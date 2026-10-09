use bream_stream_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        match try_link(self_id()) {
            LinkResult::Linked => {
                result(1);
            }
        }
    }
}
fn main() {}
