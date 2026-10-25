//! The `dama` program. `dama arena` plays matches between agents; the text protocol (position /
//! go / bestmove) comes in a later session.

mod arena;

use std::process::ExitCode;

const USAGE: &str = "usage: dama arena <A> <B> [--games N] [--seed S] [--csv PATH] [--log PATH]
       dama arena --help";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("arena") => arena::main(args),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
