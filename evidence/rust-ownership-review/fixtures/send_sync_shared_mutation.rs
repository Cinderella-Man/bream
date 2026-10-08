#![forbid(unsafe_code)]
use std::sync::{Arc,Mutex};
fn transfer<T: Send+Sync+'static>(value:T)->T {value}
fn main() { let original=Arc::new(Mutex::new(0)); let other=transfer(original.clone()); *other.lock().unwrap()+=1; assert_eq!(*original.lock().unwrap(),1); println!("shared_counter=1"); }
