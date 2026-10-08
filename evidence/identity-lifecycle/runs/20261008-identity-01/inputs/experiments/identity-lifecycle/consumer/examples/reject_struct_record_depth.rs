use bream_identity_macros::process_module;
#[process_module]
mod checked {
    struct S0 {
        next: S1,
    }
    struct S1 {
        next: S2,
    }
    struct S2 {
        next: S3,
    }
    struct S3 {
        next: S4,
    }
    struct S4 {
        value: u64,
    }
    fn run() {}
}
fn main() {}
