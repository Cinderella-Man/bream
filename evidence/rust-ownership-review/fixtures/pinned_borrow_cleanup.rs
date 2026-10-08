#![forbid(unsafe_code)]
use std::{future::{Future,pending},sync::{Arc,atomic::{AtomicUsize,Ordering}},task::{Wake,Waker,Context}};
struct Noop; impl Wake for Noop {fn wake(self:Arc<Self>) {}}
struct Guard(Arc<AtomicUsize>); impl Drop for Guard {fn drop(&mut self){self.0.fetch_add(1,Ordering::SeqCst);}}
fn main(){let count=Arc::new(AtomicUsize::new(0)); let guard_count=count.clone();let mut frame=Box::pin(async move {let _guard=Guard(guard_count);let data=vec![1u8,2,3];let view=data.as_slice(); pending::<()>().await;std::hint::black_box(view);});let waker=Waker::from(Arc::new(Noop));let mut cx=Context::from_waker(&waker);assert!(frame.as_mut().poll(&mut cx).is_pending());drop(frame);assert_eq!(count.load(Ordering::SeqCst),1);println!("pending=true drops=1");}
