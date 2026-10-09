// Modern, safe API sketch inspired by rust-lang/rust#8132, not a runtime port.
#![allow(dead_code)]
pub struct Registration<'a> {
    environment: &'a mut usize,
}

pub struct WakeTicket {
    _private: (),
}

impl<'a> Registration<'a> {
    pub fn new(environment: &'a mut usize) -> Self {
        Self { environment }
    }

    pub fn touch(&mut self) {
        *self.environment += 1;
    }

    // The ticket has no borrowed environment. Publication can start only after
    // the exclusive registration access has been consumed.
    pub fn finish(self) -> WakeTicket {
        WakeTicket { _private: () }
    }
}
