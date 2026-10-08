#![forbid(unsafe_code)]
fn must_be_static(_: &'static str) {}
fn main() { let text=String::from("owned"); must_be_static(&text); }
