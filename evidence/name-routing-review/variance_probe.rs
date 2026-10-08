use std::any::TypeId;
use std::marker::PhantomData;

// A deliberately insufficient typed-token shape, not a Bream API.
struct Covariant<T: 'static> {
    checked_type: TypeId,
    marker: PhantomData<T>,
}

fn main() {
    let original = Covariant::<fn(&())> {
        checked_type: TypeId::of::<fn(&())>(),
        marker: PhantomData,
    };
    let coerced: Covariant<fn(&'static ())> = original;
    assert_ne!(coerced.checked_type, TypeId::of::<fn(&'static ())>());
    println!("COVARIANCE_BYPASSED_CONSTRUCTION_CHECK");
}
