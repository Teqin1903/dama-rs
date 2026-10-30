//! `dama api`: the web API (docs/web-api.md) on stdin and stdout, for tests and for building the
//! site without a browser. Each line of stdin is one request, `{"call": "think", "args": ["white",
//! "{}"]}`; each answer is the JSON the WebAssembly call returns, on one line. One session lives
//! for the whole stream. The calls themselves are `engine::Session`.

use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use engine::{Json, Session, native_clock};

use crate::arena;

const HELP: &str = "\
usage: dama api

Reads one JSON request per line on stdin, {\"call\": NAME, \"args\": [...]}, and prints one JSON
answer per line: the answer of the same call in the WebAssembly build (docs/web-api.md). The
calls are version, new, reset, state, legal, play, undo, set_agent, think, analyze, eval and
position; a failed call answers {\"error\": \"...\"}.
";

/// Runs `dama api` with the arguments that follow `api`.
pub fn main(mut args: impl Iterator<Item = String>) -> ExitCode {
    match args.next().as_deref() {
        None => {}
        Some("-h" | "--help") => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Some(other) => {
            eprintln!("dama api: unexpected argument {other:?}\n\n{HELP}");
            return ExitCode::from(2);
        }
    }
    let engine = arena::code_version().unwrap_or_else(|| "unknown".to_string());
    let mut session = Session::new(native_clock);
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for line in io::stdin().lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                eprintln!("dama api: {e}");
                return ExitCode::FAILURE;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let answer = request(&mut session, &line, &engine);
        if writeln!(out, "{answer}")
            .and_then(|()| out.flush())
            .is_err()
        {
            // The reader has gone: nothing more to answer.
            return ExitCode::SUCCESS;
        }
    }
    ExitCode::SUCCESS
}

/// The answer to one request line.
fn request(session: &mut Session, line: &str, engine: &str) -> Json {
    let error = |e: String| Json::obj(vec![("error", Json::Str(e))]);
    let request = match Json::parse(line) {
        Ok(request) => request,
        Err(e) => return error(e),
    };
    let Some(name) = request.get("call").and_then(Json::as_str) else {
        return error("a request needs \"call\": the name of the call".to_string());
    };
    let args = match request.get("args") {
        None => &[][..],
        Some(Json::Arr(args)) => args.as_slice(),
        Some(_) => return error("\"args\" must be a list".to_string()),
    };
    session.call(name, args, engine)
}
