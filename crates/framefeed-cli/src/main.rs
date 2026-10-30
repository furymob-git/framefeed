//! `framefeed` command-line entry point.

#![forbid(unsafe_code)]

mod doctor;
mod play;
mod probe;
mod serve;

use std::process::ExitCode;

const USAGE: &str = "\
framefeed — video in your terminal

Usage:
  framefeed play <SOURCE> [OPTIONS]    Play a local file or a network stream
  framefeed serve [OPTIONS]            Stream terminal video over HTTP (curl mode)
  framefeed doctor                     Report terminal graphics capabilities
  framefeed probe <PLAYLIST> [OPTIONS] Inspect an HLS master playlist
  framefeed --version | --help

Play options:
  --quality <144p|240p|360p|480p|720p|1080p|auto>   Render/source quality target
  --backend <auto|kitty|ansi>                       Display backend (default: auto)
  --fps <1-60>                                      Presentation rate cap
  --fit <contain|cover|stretch>                     How the frame fills the viewport
  --stats                                           Print playback statistics on exit

Serve options:
  --listen <ADDR>        Address to bind (default 127.0.0.1:8080)
  --allow-local          Allow serving local file paths, not just http(s) URLs
  --max-streams <N>      Concurrent stream limit (default 4)

Examples:
  framefeed play ./clip.mp4 --quality 480p
  framefeed play https://example.com/master.m3u8 --quality 360p
  framefeed serve --allow-local
  curl -N 'http://127.0.0.1:8080/stream?url=./clip.mp4&q=144p'

Audio and iTerm2 output are not implemented yet — see docs/ROADMAP.md.
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
        Some("play") => play::run(args.collect()),
        Some("serve") => serve::run(args.collect()),
        Some("doctor") => doctor::run(args.collect()),
        Some("probe") => probe::run(args.collect()),
        Some(command) => {
            eprintln!("framefeed: unknown command `{command}` (see --help)");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::USAGE;

    #[test]
    fn usage_documents_every_subcommand() {
        for subcommand in ["play", "serve", "doctor", "probe"] {
            assert!(
                USAGE.contains(subcommand),
                "usage is missing `{subcommand}`"
            );
        }
    }
}
