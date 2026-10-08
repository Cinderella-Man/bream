#![forbid(unsafe_code)]
use std::{mem::ManuallyDrop,sync::{Arc,atomic::{AtomicUsize,Ordering}}};
struct Owned(Arc<AtomicUsize>);
impl Drop for Owned {fn drop(&mut self){self.0.fetch_add(1,Ordering::SeqCst);}}
fn main(){let count=Arc::new(AtomicUsize::new(0));{let _held=ManuallyDrop::new(Owned(count.clone()));} assert_eq!(count.load(Ordering::SeqCst),0); println!("drops=0");}
