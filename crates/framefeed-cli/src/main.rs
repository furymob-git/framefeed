//! `framefeed` command-line entry point.
//!
//! Milestone M0 only ships the skeleton: argument surface and exit codes are
//! defined here so the CLI contract is stable before the pipeline lands.
//! See `docs/ROADMAP.md` for the implementation milestones.

#![forbid(unsafe_code)]

use std::process::ExitCode;

const USAGE: &str = "\
framefeed — video in your terminal

Usage:
  framefeed play <SOURCE> [OPTIONS]   Play a local file or a network stream
  framefeed serve [OPTIONS]           Stream terminal video over HTTP (curl mode)
  framefeed doctor                    Report terminal graphics capabilities
  framefeed --version | --help

Sources:
  Local paths (mp4, webm, mkv, ...) and URLs (HTTP progressive, HLS, DASH).

Not implemented yet — this is the design-phase skeleton.
Track progress in docs/ROADMAP.md.
";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("-h" | "--help" | "help") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("-V" | "--version" | "version") => {
            println!(concat!("framefeed ", env!("CARGO_PKG_VERSION")));
            ExitCode::SUCCESS
        }
        Some(command) => {
            eprintln!("framefeed: `{command}` is not implemented yet (see docs/ROADMAP.md)");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::USAGE;

    #[test]
    fn usage_documents_every_planned_subcommand() {
        for subcommand in ["play", "serve", "doctor"] {
            assert!(
                USAGE.contains(subcommand),
                "usage is missing `{subcommand}`"
            );
        }
    }
}
