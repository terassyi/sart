pub(crate) mod bgp;
pub(crate) mod cmd;
pub(crate) mod fib;
pub(crate) mod agent;
pub(crate) mod proto;
pub(crate) mod trace;
pub(crate) mod util;

fn main() {
    cmd::main()
}
