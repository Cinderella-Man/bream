#![forbid(unsafe_code)]
fn transfer(value: Vec<u8>) -> Vec<u8> { value }
fn main() { let data=vec![1u8,2,3,4]; let before=data.as_ptr(); let received=transfer(data); assert_eq!(received,[1,2,3,4]); assert_eq!(before,received.as_ptr()); println!("same_allocation=true bytes={}",received.len()); }
