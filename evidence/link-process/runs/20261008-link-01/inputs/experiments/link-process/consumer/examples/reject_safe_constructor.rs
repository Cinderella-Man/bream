use bream_link_runtime::Program;
fn main() {
    let _ = Program::__from_generated(|_| Box::pin(async { loop {} }));
}
