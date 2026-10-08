use bream_shared_macros::process_module;
#[process_module]
mod rejected {
    fn run() {
        let reference = MonitorRef { value: 1 };
    }
}
fn main() {}
