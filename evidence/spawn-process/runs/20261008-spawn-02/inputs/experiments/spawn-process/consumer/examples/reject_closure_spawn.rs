use bream_spawn_runtime::Runtime;
fn main() {
    let r = Runtime::new(32);
    r.spawn(|| loop {}, vec![]).unwrap();
}
