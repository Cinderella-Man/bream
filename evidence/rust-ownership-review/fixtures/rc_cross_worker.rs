#![forbid(unsafe_code)]
fn transferable<T: Send + 'static>(_: T) {}
fn main() { transferable(std::rc::Rc::new(1u8)); }
