use bream_read_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let exit = Exit {
            from: self_id(),
            reason: 0,
        };
    }
}
fn main() {}
