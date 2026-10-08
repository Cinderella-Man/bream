use bream_mailbox_macros::process_module;
#[process_module]
mod checked {
    struct Leaf {
        f0: u64,
        f1: u64,
        f2: u64,
        f3: u64,
        f4: u64,
        f5: u64,
        f6: u64,
        f7: u64,
    }
    struct Wide {
        f0: Leaf,
        f1: Leaf,
        f2: Leaf,
        f3: Leaf,
        f4: Leaf,
        f5: Leaf,
        f6: Leaf,
        f7: Leaf,
    }
    fn run() {}
}
fn main() {}
