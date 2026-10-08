#![forbid(unsafe_code)]
use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
struct Owned { counter:Arc<AtomicUsize>, _storage:Vec<u8> }
impl Drop for Owned {fn drop(&mut self){self.counter.fetch_add(1,Ordering::SeqCst);}}
fn main(){let count=Arc::new(AtomicUsize::new(0)); std::mem::forget(Owned{counter:count.clone(),_storage:vec![0;8]}); assert_eq!(count.load(Ordering::SeqCst),0); println!("drops=0");}
