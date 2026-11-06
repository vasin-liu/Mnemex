//! Mnemex CLI — scaffold-stage entry point.
//!
//! Phase 0 carrier for index/verify commands (the main manual entry until
//! the Tauri shell lands). No third-party dependencies: argument handling
//! is std-only.

use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("-h") | Some("--help") => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("-V") | Some("--version") => {
            println!("mnemex {VERSION}");
            ExitCode::SUCCESS
        }
        Some(cmd) => {
            eprintln!("mnemex: unknown command `{cmd}`");
            print_help();
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        "mnemex {VERSION} — local engineering memory engine (scaffold stage)\n\
         \n\
         Usage: mnemex [--version | --help]\n\
         \n\
         Scope: the workspace path (plus optional branch/commit) that callers\n\
         pass explicitly on every request. Mnemex never mints scope handles;\n\
         server-minted handles are reserved for real server state only\n\
         (index snapshots, pagination cursors). See docs/adr/ADR-006."
    );
}
