//! The `duscope` binary. Until the command-line interface lands (P1-CLI-01) it
//! is a pre-release placeholder that answers `--version` and nothing else.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::io::{self, Write};
use std::process::ExitCode;

fn main()->ExitCode{
    let mut args = std::env::args_os().skip(1);
    match (args.next(), args.next()) {
        (Some(arg), None) if arg == "--version" => print_version(),
        _ => {
            eprintln!(
                "duscope {} is a pre-release placeholder: only `duscope --version` is implemented",
                env!("CARGO_PKG_VERSION")
            );
            ExitCode::from(2)
        }
    }
}

/// Writes the version line to stdout; a closed pipe exits 0 quietly.
fn print_version() -> ExitCode {
    let mut stdout = io::stdout().lock();
    let written =
        writeln!(stdout, "duscope {}", env!("CARGO_PKG_VERSION")).and_then(|()| stdout.flush());
    match written {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
