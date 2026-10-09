extern crate bream_timed_runtime as actual_runtime;
extern crate self as bream_timed_runtime;
extern crate self as core;
extern crate self as std;
extern crate std as actual_std;
pub struct StreamBytes;
pub struct StreamResult;
pub struct TimedStreamResult;
pub mod primitive {
    #[allow(non_camel_case_types)]
    pub type u64 = u8;
    #[allow(non_camel_case_types)]
    pub type bool = u8;
}
use bream_timed_macros::process_module;
#[process_module]
mod checked {
    struct Pair {
        first: StreamBytes,
        second: StreamBytes,
    }
    fn hold(pair: Pair) {
        let _gate = receive(100);
        let mut sum: u64 = 0;
        for i in 0..stream_len(pair.first) {
            sum = sum + stream_byte(pair.first, i);
        }
        for j in 0..stream_len(pair.second) {
            sum = sum + stream_byte(pair.second, j);
        }
        result(sum);
    }
    fn next(first: StreamBytes) {
        match read_next_for(5) {
            TimedStreamResult::Data(bytes) => {
                hold(Pair {
                    first: first,
                    second: bytes,
                });
            }
            TimedStreamResult::Eof => {
                result(900);
            }
            TimedStreamResult::Failed => {
                result(901);
            }
            TimedStreamResult::Unavailable => {
                result(902);
            }
            TimedStreamResult::Timeout => {
                result(904);
            }
            TimedStreamResult::Busy => {
                result(903);
            }
        }
    }
    fn run() {
        let _gate = receive(99);
        match read_next_for(5) {
            TimedStreamResult::Data(bytes) => {
                next(bytes);
            }
            TimedStreamResult::Eof => {
                result(900);
            }
            TimedStreamResult::Failed => {
                result(901);
            }
            TimedStreamResult::Unavailable => {
                result(902);
            }
            TimedStreamResult::Timeout => {
                result(904);
            }
            TimedStreamResult::Busy => {
                result(903);
            }
        }
    }
}
fn main() {
    use actual_std::io::Write;
    let rt = actual_runtime::Runtime::with_stream_capacity(1, 2, 1);
    let h = rt.spawn(checked::program(), actual_std::vec![]).unwrap();
    let (reader, mut writer) = actual_std::os::unix::net::UnixStream::pair().unwrap();
    let r = rt.client().install_stream(h.id(), reader, 3).unwrap();
    writer.write_all(&[2, 3, 5, 7, 11, 13]).unwrap();
    for tag in [99, 100] {
        rt.client().send(
            h.id(),
            actual_runtime::Envelope::new(tag, actual_std::vec![])
                .unwrap()
                .0,
        );
    }
    let deadline = actual_std::time::Instant::now() + actual_std::time::Duration::from_secs(30);
    loop {
        if h.snapshot().cleanup_event > 0 && r.snapshot().retired {
            break;
        }
        assert!(
            actual_std::time::Instant::now() < deadline,
            "correctness watchdog"
        );
        actual_std::thread::yield_now();
    }
    assert_eq!((h.snapshot().terminal, h.snapshot().result), (1, 41));
    assert_eq!(
        (
            r.snapshot().started,
            r.snapshot().delivered,
            r.snapshot().drops,
            r.snapshot().live_bytes,
            r.snapshot().endpoint_drops
        ),
        (2, 2, 2, 0, 1)
    );
    assert_eq!(rt.client().clock_snapshot().pending, 0);
    assert_eq!(rt.client().clock_snapshot().armed, 2);
    assert_eq!(rt.client().clock_snapshot().cancelled, 2);
    rt.shutdown();
    actual_std::println!("SHADOW_TIMED_READ_PASSED");
}
