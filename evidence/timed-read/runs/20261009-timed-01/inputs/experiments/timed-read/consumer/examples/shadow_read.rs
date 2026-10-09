extern crate bream_timed_runtime as actual_runtime;
extern crate self as bream_timed_runtime;
extern crate self as core;
extern crate self as std;
extern crate std as actual_std;
pub struct ReadBytes;
pub struct ReadResult;
pub mod primitive {
    #[allow(non_camel_case_types)]
    pub type u64 = u8;
    #[allow(non_camel_case_types)]
    pub type bool = u8;
}
use bream_timed_macros::process_module;
#[process_module]
mod checked {
    struct Owner {
        payload: ReadBytes,
    }
    fn checksum(state: Owner) {
        let _pause = receive(100);
        let mut sum: u64 = 0;
        for i in 0..read_len(state.payload) {
            sum = sum * 31 + read_byte(state.payload, i);
        }
        result(sum);
    }
    fn run() {
        let _gate = receive(99);
        match read_input() {
            ReadResult::Data(bytes) => {
                checksum(Owner { payload: bytes });
            }
            ReadResult::Eof => {
                result(900);
            }
            ReadResult::Failed => {
                result(901);
            }
            ReadResult::Unavailable => {
                result(902);
            }
        }
    }
}
fn main() {
    use actual_std::io::Write;
    let rt = actual_runtime::Runtime::with_read_capacity(1, 2, 1);
    let actor = rt.spawn(checked::program(), actual_std::vec![]).unwrap();
    let (reader, mut writer) = actual_std::os::unix::net::UnixStream::pair().unwrap();
    let receipt = rt.client().install_read(actor.id(), reader, 8).unwrap();
    writer.write_all(&[2, 3, 5, 7]).unwrap();
    for tag in [99, 100] {
        rt.client().send(
            actor.id(),
            actual_runtime::Envelope::new(tag, actual_std::vec![])
                .unwrap()
                .0,
        );
    }
    let deadline = actual_std::time::Instant::now() + actual_std::time::Duration::from_secs(30);
    loop {
        if actor.snapshot().cleanup_event > 0 && receipt.snapshot().retired {
            break;
        }
        assert!(
            actual_std::time::Instant::now() < deadline,
            "correctness watchdog"
        );
        actual_std::thread::yield_now();
    }
    assert_eq!(
        (actor.snapshot().terminal, actor.snapshot().result),
        (1, 62627)
    );
    assert_eq!(
        (
            receipt.snapshot().live_bytes,
            receipt.snapshot().storage_drops
        ),
        (0, 1)
    );
    rt.shutdown();
    actual_std::println!("SHADOW_READ_PASSED");
}
