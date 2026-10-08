use bream_room_macros::process;

#[process]
fn forbidden() {
    unsafe { loop {} }
}

fn main() {}
