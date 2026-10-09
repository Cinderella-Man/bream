use bream_exit_macros::process_module;
#[process_module]
mod checked {
    fn f0(value: u64) -> u64 {
        return f1(value);
    }
    fn f1(value: u64) -> u64 {
        return f2(value);
    }
    fn f2(value: u64) -> u64 {
        return f3(value);
    }
    fn f3(value: u64) -> u64 {
        return f4(value);
    }
    fn f4(value: u64) -> u64 {
        return f5(value);
    }
    fn f5(value: u64) -> u64 {
        return f6(value);
    }
    fn f6(value: u64) -> u64 {
        return f7(value);
    }
    fn f7(value: u64) -> u64 {
        return f8(value);
    }
    fn f8(value: u64) -> u64 {
        return value;
    }
    fn run() {}
}
fn main() {}
