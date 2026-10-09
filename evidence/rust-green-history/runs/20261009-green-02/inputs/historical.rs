// Compiled only with the archived Rust 0.12 compiler. Correctness, no timing.
extern crate green;

use green::{SchedPool, PoolConfig, GreenTaskBuilder};
use std::sync::Arc;
use std::sync::atomic::{AtomicUint, SeqCst};
use std::task;
use std::task::TaskBuilder;

local_data_key!(actor_id: uint)

fn pool() -> SchedPool {
    SchedPool::new(PoolConfig {
        threads: 1,
        event_loop_factory: green::basic::event_loop,
    })
}

// The host publishes a second task while the sole green scheduler executes A.
// Atomics are deliberate: 0.12 channel operations can implicitly yield.
fn no_yield_gate() {
    let mut p = pool();
    let started = Arc::new(AtomicUint::new(0));
    let released = Arc::new(AtomicUint::new(0));
    let phase = Arc::new(AtomicUint::new(0));
    let s = started.clone();
    let r = released.clone();
    let a = phase.clone();
    let first = TaskBuilder::new().green(&mut p).try_future(proc() {
        a.store(1, SeqCst);
        s.store(1, SeqCst);
        while r.load(SeqCst) == 0 {}
        // No channel, allocation, I/O, or explicit yield in the gated phase.
        a.store(2, SeqCst);
        7u
    });
    while started.load(SeqCst) == 0 { task::deschedule(); }
    let peer_phase = phase.clone();
    let peer = TaskBuilder::new().green(&mut p).try_future(proc() {
        peer_phase.load(SeqCst)
    });
    released.store(1, SeqCst);
    assert_eq!(first.unwrap().ok().unwrap(), 7);
    assert_eq!(peer.unwrap().ok().unwrap(), 2);
    p.shutdown();
    println!("{{\"case\":\"no_yield_gate\",\"peer_saw_finished_phase\":true}}");
}

fn cooperative_yield() {
    let mut p = pool();
    let started = Arc::new(AtomicUint::new(0));
    let released = Arc::new(AtomicUint::new(0));
    let phase = Arc::new(AtomicUint::new(0));
    let s = started.clone();
    let r = released.clone();
    let a = phase.clone();
    let first = TaskBuilder::new().green(&mut p).try_future(proc() {
        let stack_sentinel = [13u, 21, 34, 55];
        a.store(1, SeqCst);
        s.store(1, SeqCst);
        while r.load(SeqCst) == 0 { task::deschedule(); }
        assert!(stack_sentinel == [13u, 21, 34, 55]);
        a.store(2, SeqCst);
        7u
    });
    while started.load(SeqCst) == 0 { task::deschedule(); }
    let peer_phase = phase.clone();
    let peer_release = released.clone();
    let peer = TaskBuilder::new().green(&mut p).try_future(proc() {
        let observed = peer_phase.load(SeqCst);
        peer_release.store(1, SeqCst);
        observed
    });
    assert_eq!(peer.unwrap().ok().unwrap(), 1);
    assert_eq!(first.unwrap().ok().unwrap(), 7);
    p.shutdown();
    println!("{{\"case\":\"cooperative_yield\",\"peer_saw_active_phase\":true,\"stack_preserved\":true}}");
}

fn channel_and_task_local() {
    let mut p = pool();
    let result = TaskBuilder::new().green(&mut p).try_future(proc() {
        actor_id.replace(Some(11));
        let (tx, rx) = channel();
        task::spawn(proc() {
            assert!(actor_id.get().is_none());
            actor_id.replace(Some(22));
            task::deschedule();
            tx.send(*actor_id.get().unwrap());
        });
        // Blocking green receive leaves the single scheduler available to child.
        assert_eq!(rx.recv(), 22);
        assert_eq!(*actor_id.get().unwrap(), 11);
        7u
    });
    assert_eq!(result.unwrap().ok().unwrap(), 7);
    p.shutdown();
    println!("{{\"case\":\"channel_and_task_local\",\"child_ran\":true,\"locals_isolated\":true}}");
}

struct Guard { drops: Arc<AtomicUint> }
impl Drop for Guard {
    fn drop(&mut self) { self.drops.fetch_add(1, SeqCst); }
}

fn panic_cleanup() {
    let mut p = pool();
    let drops = Arc::new(AtomicUint::new(0));
    let owned = drops.clone();
    let failed = TaskBuilder::new().green(&mut p).try_future(proc() -> () {
        let _guard = Guard { drops: owned };
        fail!("intentional historical task panic");
    });
    let peer = TaskBuilder::new().green(&mut p).try_future(proc() { 17u });
    assert!(failed.unwrap().is_err());
    assert_eq!(drops.load(SeqCst), 1);
    assert_eq!(peer.unwrap().ok().unwrap(), 17);
    p.shutdown();
    println!("{{\"case\":\"panic_cleanup\",\"drops\":1,\"peer_reply\":17}}");
}

fn main() {
    let args = std::os::args();
    match args[1].as_slice() {
        "no_yield_gate" => no_yield_gate(),
        "cooperative_yield" => cooperative_yield(),
        "channel_and_task_local" => channel_and_task_local(),
        "panic_cleanup" => panic_cleanup(),
        _ => fail!("unknown case"),
    }
}
