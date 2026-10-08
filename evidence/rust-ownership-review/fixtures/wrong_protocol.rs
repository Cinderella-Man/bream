#![forbid(unsafe_code)]
struct Address<M>(std::marker::PhantomData<fn(M)>);
fn send<M>(_: &Address<M>, _: M) {}
enum Room { Join }
enum Session { Ping }
fn main() { let room=Address::<Room>(std::marker::PhantomData); send(&room,Session::Ping); }
