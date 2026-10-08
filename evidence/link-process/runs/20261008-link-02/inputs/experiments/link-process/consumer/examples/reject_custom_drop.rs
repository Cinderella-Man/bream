use bream_link_macros::process;

#[process]
fn forbidden() {
    struct Custom;
    impl Drop for Custom {
        fn drop(&mut self) {
            loop {}
        }
    }
    let guard = Custom;
}

fn main() {}
