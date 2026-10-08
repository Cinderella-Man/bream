use bream_replacement_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let value = down_reason(monitor(self_id()));
    }
}
fn main() {}
