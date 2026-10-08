use bream_room_macros::process_module;
#[process_module]
mod rejected {
    fn child() {} fn run() { spawn_link_monitor(child, 0, 0, 0); }
}
fn main() {}
