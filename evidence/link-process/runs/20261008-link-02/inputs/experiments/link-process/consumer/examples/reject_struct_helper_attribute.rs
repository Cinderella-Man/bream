use bream_link_macros::process_module;
#[process_module]
mod checked {
    #[inline]
    fn helper() {}
    fn run() {}
}
fn main() {}
