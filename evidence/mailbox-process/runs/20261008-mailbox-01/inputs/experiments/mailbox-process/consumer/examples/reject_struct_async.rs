use bream_mailbox_macros::process_module;
#[process_module]
mod checked {
    async fn helper() {}
    fn run() {}
}
fn main() {}
