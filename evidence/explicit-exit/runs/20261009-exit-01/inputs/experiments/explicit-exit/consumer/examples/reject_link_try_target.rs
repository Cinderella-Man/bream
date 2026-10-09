use bream_exit_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let outcome = try_link(true);
    }
}
fn main() {}
