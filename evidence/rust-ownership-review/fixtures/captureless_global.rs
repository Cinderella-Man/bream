#![forbid(unsafe_code)]
use std::sync::atomic::{AtomicUsize,Ordering};
static SHARED:AtomicUsize=AtomicUsize::new(0);
fn call<F:FnOnce()+Send+Sync+'static>(f:F){f();}
fn main(){call(|| {SHARED.fetch_add(1,Ordering::SeqCst);}); assert_eq!(SHARED.load(Ordering::SeqCst),1);println!("global=1");}
