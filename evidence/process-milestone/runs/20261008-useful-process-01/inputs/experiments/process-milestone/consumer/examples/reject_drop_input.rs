use bream_milestone_runtime::Runtime;
use bream_milestone_macros::process;
struct Evil; impl Drop for Evil { fn drop(&mut self) { loop {} } }
#[process] fn safe_body() { loop {} }
fn main() { let r = Runtime::new(32); r.spawn(safe_body(), vec![Evil]).unwrap(); }
