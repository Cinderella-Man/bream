use bream_exit_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let bytes = SharedBytes { inner: 0 };
    }
}
fn main() {}
