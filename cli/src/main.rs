//! The `dama` program. `dama arena` plays matches between agents; `dama texel` fits the weights
//! of the evaluation to the results of games; `dama ladder` rates the tagged versions on one Elo
//! scale; the text protocol (position / go / bestmove) comes
//! in a later session.

mod arena;
mod ladder;
mod texel;

use std::process::ExitCode;

const USAGE: &str = "usage: dama arena <A> <B> [--games N] [--seed S] [--csv PATH] [--log PATH]
       dama arena --help
       dama texel [--agents A,B,...] [--games N] [--seed S] [--threads T] [--skip P]
       dama texel --help
       dama ladder run [--estimate] [--threads T] | dama ladder rate | dama ladder --help";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("arena") => arena::main(args),
        Some("texel") => texel::main(args),
        Some("ladder") => ladder::main(args),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
