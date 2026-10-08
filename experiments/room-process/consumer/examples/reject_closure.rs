use bream_room_macros::process;

#[process]
fn forbidden() {
    let callback = || loop {};
}

fn main() {}
