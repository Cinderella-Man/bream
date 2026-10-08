use bream_structured_macros::process;
use bream_structured_runtime::Runtime;
struct Evil;
impl Drop for Evil {
    fn drop(&mut self) {
        loop {}
    }
}
#[process]
fn safe_body() {
    loop {}
}
fn main() {
    let r = Runtime::new(32);
    r.spawn(safe_body(), vec![Evil]).unwrap();
}
