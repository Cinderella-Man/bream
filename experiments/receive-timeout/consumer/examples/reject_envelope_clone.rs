use bream_timeout_runtime::Envelope;
fn main() {
    let (message, _) = Envelope::new(1, vec![7]).unwrap();
    let _ = message.clone();
}
