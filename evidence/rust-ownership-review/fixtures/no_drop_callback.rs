#![forbid(unsafe_code)]
#[derive(Clone,Copy)]
struct Payload { callback:fn() }
fn spinning(){loop{std::hint::spin_loop();}}
fn main(){let payload=Payload{callback:spinning}; assert!(!std::mem::needs_drop::<Payload>()); (payload.callback)();}
