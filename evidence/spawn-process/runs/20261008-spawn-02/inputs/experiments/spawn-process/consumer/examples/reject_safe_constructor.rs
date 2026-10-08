use bream_spawn_runtime::Program;
fn main() {
    let _ = Program::__from_generated(|_| Box::pin(async { loop {} }));
}
