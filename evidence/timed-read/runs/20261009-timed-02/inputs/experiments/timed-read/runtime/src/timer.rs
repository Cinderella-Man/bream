use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockError {
    Backwards,
    Closed,
    DeadlineOverflow,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClockSnapshot {
    pub now: u128,
    pub pending: usize,
    pub armed: u64,
    pub cancelled: u64,
    pub expired: u64,
    pub stale: u64,
    pub service_batches: u64,
    pub callbacks: u64,
    pub polls_with_due: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Receive,
    Stream,
}
#[derive(Clone)]
pub(super) enum Target {
    Receive(Weak<Metrics>),
    Stream(Weak<stream_read::Stream>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Key {
    pub(super) deadline: u128,
    pub(super) process: ProcessId,
    pub(super) generation: u64,
    kind: Kind,
}
#[derive(Default)]
struct State {
    counters: ClockSnapshot,
    entries: BTreeMap<Key, Target>,
}
#[derive(Default)]
pub(super) struct Clock {
    state: Mutex<State>,
}
pub(super) struct Registration {
    pub(super) key: Key,
    pub(super) expired: bool,
    clock: Arc<Clock>,
}
impl Drop for Registration {
    fn drop(&mut self) {
        // Remove eagerly: repeated match/cancel must not retain tombstones until
        // their deadlines. Detached callbacks still validate this exact key.
        let mut state = self.clock.state.lock().unwrap();
        if state.entries.remove(&self.key).is_some() {
            state.counters.cancelled += 1;
        }
    }
}
impl Clock {
    pub(super) fn application_poll(&self) {
        let mut state = self.state.lock().unwrap();
        if state
            .entries
            .first_key_value()
            .is_some_and(|(key, _)| key.deadline <= state.counters.now)
        {
            state.counters.polls_with_due += 1;
        }
    }
    pub(super) fn snapshot(&self) -> ClockSnapshot {
        let state = self.state.lock().unwrap();
        ClockSnapshot {
            pending: state.entries.len(),
            ..state.counters
        }
    }
    pub(super) fn advance_to(&self, now: u128) -> Result<(), ClockError> {
        let mut state = self.state.lock().unwrap();
        if now < state.counters.now {
            return Err(ClockError::Backwards);
        }
        state.counters.now = now;
        Ok(())
    }
    pub(super) fn arm(
        self: &Arc<Self>,
        metrics: &Arc<Metrics>,
        generation: u64,
        delay: u64,
    ) -> Result<Registration, ClockError> {
        self.insert(
            *metrics.id.get().expect("published timer owner"),
            generation,
            delay,
            Kind::Receive,
            Target::Receive(Arc::downgrade(metrics)),
        )
    }
    pub(super) fn arm_stream(
        self: &Arc<Self>,
        owner: ProcessId,
        generation: u64,
        delay: u64,
        stream: Weak<stream_read::Stream>,
    ) -> Result<Registration, ClockError> {
        self.insert(
            owner,
            generation,
            delay,
            Kind::Stream,
            Target::Stream(stream),
        )
    }
    fn insert(
        self: &Arc<Self>,
        process: ProcessId,
        generation: u64,
        delay: u64,
        kind: Kind,
        target: Target,
    ) -> Result<Registration, ClockError> {
        let mut state = self.state.lock().unwrap();
        let deadline = state
            .counters
            .now
            .checked_add(u128::from(delay))
            .ok_or(ClockError::DeadlineOverflow)?;
        let key = Key {
            deadline,
            process,
            generation,
            kind,
        };
        assert!(state.entries.insert(key, target).is_none());
        state.counters.armed += 1;
        Ok(Registration {
            key,
            expired: false,
            clock: self.clone(),
        })
    }
    pub(super) fn has_due(&self) -> bool {
        let state = self.state.lock().unwrap();
        state
            .entries
            .first_key_value()
            .is_some_and(|(key, _)| key.deadline <= state.counters.now)
    }
    pub(super) fn take_due(&self) -> Option<(Key, Target)> {
        let mut state = self.state.lock().unwrap();
        if state
            .entries
            .first_key_value()
            .is_some_and(|(key, _)| key.deadline <= state.counters.now)
        {
            state.entries.pop_first()
        } else {
            None
        }
    }
    pub(super) fn expire(&self, key: Key, target: Target) {
        // Never hold the clock lock while acquiring a mailbox: arm/cancel uses
        // mailbox -> clock. A record hold protects lifetime, not live identity.
        let accepted = match target {
            Target::Stream(stream) => stream.upgrade().is_some_and(|stream| stream.expire(key)),
            Target::Receive(record) => record.upgrade().is_some_and(|metrics| {
                let mut inbox = metrics.mailbox.lock().unwrap();
                if inbox.closed
                    || !inbox
                        .timer
                        .as_ref()
                        .is_some_and(|timer| timer.key == key && !timer.expired)
                {
                    return false;
                }
                inbox.timer.as_mut().unwrap().expired = true;
                inbox.waiting = false;
                drop(inbox);
                if let Some(shared) = metrics.runtime.get().and_then(Weak::upgrade) {
                    shared.wake(key.process);
                }
                true
            }),
        };
        let mut state = self.state.lock().unwrap();
        state.counters.callbacks += 1;
        if accepted {
            state.counters.expired += 1;
        } else {
            state.counters.stale += 1;
        }
    }
    pub(super) fn service(&self, limit: usize) -> usize {
        let mut count = 0;
        while count < limit {
            let Some((key, record)) = self.take_due() else {
                break;
            };
            self.expire(key, record);
            count += 1;
        }
        if count > 0 {
            self.state.lock().unwrap().counters.service_batches += 1;
        }
        count
    }
}
