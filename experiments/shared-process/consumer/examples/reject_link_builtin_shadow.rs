use bream_shared_macros::process_module;
#[process_module]
mod rejected {
    fn try_link(target: Pid) {
        unlink(target);
    }
    fn run() {
        try_link(self_id());
    }
}
fn main() {}
