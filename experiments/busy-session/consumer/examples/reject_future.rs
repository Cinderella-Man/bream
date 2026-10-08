use bream_busy_runtime::Runtime;
fn main() {
    let r = Runtime::new(32);
    r.spawn(async { loop {} }, vec![]).unwrap();
}
