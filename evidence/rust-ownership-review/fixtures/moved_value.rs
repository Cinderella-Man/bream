#![forbid(unsafe_code)]
fn main() { let value=vec![1u8,2,3]; let receiver=value; println!("{} {}",receiver.len(),value.len()); }
