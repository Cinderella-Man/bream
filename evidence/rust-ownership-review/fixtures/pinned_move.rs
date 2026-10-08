#![forbid(unsafe_code)]
use std::{marker::PhantomPinned,pin::Pin};
struct State { _pin: PhantomPinned }
fn main() { let pinned=Box::pin(State { _pin:PhantomPinned }); let _movable=Pin::into_inner(pinned); }
