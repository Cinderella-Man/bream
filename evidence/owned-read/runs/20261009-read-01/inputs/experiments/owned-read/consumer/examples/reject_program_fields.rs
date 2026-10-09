use bream_read_runtime::Program;
fn main() {
    let _ = Program {
        make: |_| Box::pin(async { loop {} }),
    };
}
