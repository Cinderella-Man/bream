use bream_room_macros::process;

#[process]
fn forbidden() {
    let r#__bream_context: u64 = 0;
}

fn main() {}
