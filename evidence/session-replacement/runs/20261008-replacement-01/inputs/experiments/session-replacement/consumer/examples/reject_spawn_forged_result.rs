use bream_replacement_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let child = Spawned {
            pid: self_id(),
            reference: monitor(self_id()),
        };
    }
}
fn main() {}
