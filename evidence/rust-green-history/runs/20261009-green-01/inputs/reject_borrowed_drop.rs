// Expected E0505: a live callback borrow prevents destroying its buffer.
#![forbid(unsafe_code)]
fn main() {
    let buffer = vec![7u8];
    std::thread::scope(|scope| {
        let borrowed = &buffer;
        let callback = scope.spawn(move || borrowed[0]);
        drop(buffer);
        assert_eq!(callback.join().unwrap(), 7);
    });
}
