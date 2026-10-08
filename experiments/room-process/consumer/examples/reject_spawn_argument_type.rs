use bream_room_macros::process_module;
#[process_module]
mod rejected {
    fn child() {} fn run() { spawn_link(child, owned_input(), 0, self_id()); }
}
fn main() {}
