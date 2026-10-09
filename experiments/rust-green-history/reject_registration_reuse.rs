// Expected E0382: finishing consumes permission to access the environment.
#![forbid(unsafe_code)]
mod registration;
use registration::Registration;
fn main() {
    let mut environment = 0;
    let mut registration = Registration::new(&mut environment);
    let _publishable_ticket = registration.finish();
    registration.touch();
}
