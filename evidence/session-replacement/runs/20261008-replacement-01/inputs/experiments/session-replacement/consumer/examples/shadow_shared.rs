extern crate bream_replacement_runtime as actual_runtime;
extern crate self as bream_replacement_runtime;
extern crate self as core;
extern crate self as std;
extern crate std as actual_std;
pub struct SharedBytes;
pub struct SharedEnvelope;
pub struct SharedReceiveResult;
pub struct Event;
pub struct ProcessId;
pub mod option {
    pub struct Option;
}
pub mod primitive {
    #[allow(non_camel_case_types)]
    pub type u64 = u8;
}
use bream_replacement_macros::process_module;
#[process_module]
mod checked {
    struct State {
        payload: SharedBytes,
        mirror: SharedBytes,
        input: Words,
    }
    fn control(expected: u64) {
        let message = receive_any();
        if message_tag(message) != expected {
            // The controls carry empty Words: wrong argument order must fail.
            let words = message_words(message);
            result(read(words, 9999));
        }
    }
    fn destination() -> Pid {
        control(11);
        return self_id();
    }
    fn tagged() -> u64 {
        control(22);
        return 777;
    }
    fn packet(bytes: SharedBytes) -> SharedBytes {
        control(33);
        return bytes;
    }
    fn hold(state: State) -> SharedBytes {
        mark(70);
        let gate = receive(99);
        if message_tag(gate) != 99 || read(state.input, 0) != 42 {
            result(read(state.input, 9999));
        }
        if shared_len(state.payload) != 3 || shared_read(state.mirror, 0) != 41 {
            result(shared_read(state.payload, 9999));
        }
        send_shared(destination(), tagged(), packet(clone_shared(state.payload)));
        match receive_shared_for(777, 0) {
            SharedReceiveResult::Message(delivered) => {
                if shared_tag(delivered) != 777 || shared_tag(delivered) != 777 {
                    result(read(state.input, 9999));
                }
                let received = shared_bytes(delivered);
                if shared_len(received) != 3 || shared_read(received, 2) != 43 {
                    result(shared_read(received, 9999));
                }
            }
            SharedReceiveResult::Timeout => {
                result(read(state.input, 9999));
            }
        }
        send_shared(self_id(), 778, state.mirror);
        let mirrored = receive_shared_any();
        if shared_tag(mirrored) != 778 {
            result(read(state.input, 9999));
        }
        let mirrored_bytes = shared_bytes(mirrored);
        if shared_read(mirrored_bytes, 1) != 42 {
            result(shared_read(mirrored_bytes, 9999));
        }
        return state.payload;
    }
    fn run() {
        let first = receive_shared(7);
        if shared_tag(first) != 7 || shared_tag(first) != 7 {
            result(999);
            return;
        }
        let bytes = shared_bytes(first);
        let duplicate = clone_shared(bytes);
        let state = State {
            payload: bytes,
            mirror: duplicate,
            input: owned_input(),
        };
        let retained = hold(state);
        match receive_shared_any_for(0) {
            SharedReceiveResult::Message(unexpected_any) => {
                result(999);
                return;
            }
            SharedReceiveResult::Timeout => {}
        }
        match receive_shared_for(888, 0) {
            SharedReceiveResult::Message(unexpected_tagged) => {
                result(999);
                return;
            }
            SharedReceiveResult::Timeout => {}
        }
        send_shared(self_id(), 779, retained);
        match receive_event() {
            Event::Shared(last) => {
                let tag = shared_tag(last);
                let last_bytes = shared_bytes(last);
                result(tag + shared_len(last_bytes) + shared_read(last_bytes, 0));
            }
            Event::Message(unexpected_words) => {
                result(999);
            }
            Event::Down(unexpected_down) => {
                result(999);
            }
            Event::Exit(unexpected_exit) => {
                result(999);
            }
        }
    }
}
fn main() {
    let rt = actual_runtime::Runtime::paused(1);
    let h = rt.spawn(checked::program(), actual_std::vec![42]).unwrap();
    let (bytes, receipt) = actual_runtime::SharedBytes::new(actual_std::vec![41, 42, 43]).unwrap();
    rt.client()
        .send_shared(h.id(), actual_runtime::SharedEnvelope::new(7, bytes));
    rt.start();
    let deadline = actual_std::time::Instant::now() + actual_std::time::Duration::from_secs(30);
    loop {
        let snapshot = h.snapshot();
        if snapshot.stage == 70 && snapshot.waiting && snapshot.parks > 0 {
            break;
        }
        assert_eq!(snapshot.terminal, 0, "must suspend with the owning record");
        assert!(
            actual_std::time::Instant::now() < deadline,
            "correctness watchdog"
        );
        actual_std::thread::yield_now();
    }
    let held = receipt.snapshot();
    assert_eq!(
        (
            held.live_bytes,
            held.physical_drops,
            held.live_references,
            held.references_created
        ),
        (3, 0, 2, 2)
    );
    for tag in [11, 22, 33, 99] {
        let (message, _) = actual_runtime::Envelope::new(tag, actual_std::vec![]).unwrap();
        rt.client().send(h.id(), message);
    }
    loop {
        let snapshot = h.snapshot();
        if snapshot.cleanup_event > 0 {
            assert_eq!(
                (
                    snapshot.terminal,
                    snapshot.result,
                    snapshot.drops,
                    snapshot.freed_bytes
                ),
                (1, 823, 1, 8)
            );
            assert_eq!((snapshot.receives, snapshot.timeouts), (8, 2));
            break;
        }
        assert!(
            actual_std::time::Instant::now() < deadline,
            "correctness watchdog"
        );
        actual_std::thread::yield_now();
    }
    rt.shutdown();
    let finished = receipt.snapshot();
    assert_eq!(
        (
            finished.live_bytes,
            finished.freed_bytes,
            finished.physical_drops
        ),
        (0, 3, 1)
    );
    assert_eq!(
        (
            finished.live_references,
            finished.references_created,
            finished.references_dropped
        ),
        (0, 3, 3)
    );
    actual_std::println!("SHADOW_SHARED_PASSED");
}
