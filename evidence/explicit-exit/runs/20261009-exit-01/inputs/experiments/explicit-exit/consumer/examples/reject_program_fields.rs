use bream_exit_runtime::Program;
fn main() {
    let _ = Program {
        make: |_| Box::pin(async { loop {} }),
    };
}
