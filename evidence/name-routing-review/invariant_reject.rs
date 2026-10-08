use std::marker::PhantomData;

struct Invariant<T>(PhantomData<fn(T) -> T>);

fn invalid_conversion(value: Invariant<fn(&())>) -> Invariant<fn(&'static ())> {
    value
}

fn main() {}
