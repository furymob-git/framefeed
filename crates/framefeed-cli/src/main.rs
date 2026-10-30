//! `framefeed` command-line entry point.
//!
//! Landed so far: `doctor` (hint-based terminal capability report) and `probe`
//! (HLS master playlist inspection and quality selection). `play` and `serve`
//! are stubs until their milestones land; see `docs/ROADMAP.md`.

#![forbid(unsafe_code)]

use std::process::ExitCode;

use framefeed_core::{DEFAULT_CELL, Viewport};
use framefeed_proto::{Hints, Multiplexer, detect};
use framefeed_source::{Quality, parse_master_playlist, resolve_uri, select_variant};

const USAGE: &str = "\
framefeed — video in your terminal

Usage:
  framefeed play <SOURCE> [OPTIONS]    Play a local file or a network stream
  framefeed serve [OPTIONS]            Stream terminal video over HTTP (curl mode)
  framefeed doctor                     Report terminal graphics capabilities
  framefeed probe <PLAYLIST> [OPTIONS] Inspect an HLS master playlist
  framefeed --version | --help

Probe options:
  --quality <144p|240p|360p|480p|720p|1080p|auto>   Preview the variant choice

Sources:
  Local paths (mp4, webm, mkv, ...) and URLs (HTTP progressive, HLS, DASH).

`play` and `serve` are not implemented yet — see docs/ROADMAP.md.
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
        Some("doctor") => doctor(),
        Some("probe") => probe(args.collect()),
        Some(command) => {
            eprintln!("framefeed: `{command}` is not implemented yet (see docs/ROADMAP.md)");
            ExitCode::from(2)
        }
    }
}

/// Prints what we know about the current terminal and the resulting budgets.
fn doctor() -> ExitCode {
    let hints = Hints::from_env();
    let detection = detect(&hints);
    let viewport = viewport_from_env();
    let raster = viewport.pixel_size();
    let budget = detection.backend.budget();

    println!(
        "framefeed {} — terminal capability report\n",
        env!("CARGO_PKG_VERSION")
    );
    println!("Environment");
    print_env("TERM", hints.term.as_deref());
    print_env("TERM_PROGRAM", hints.term_program.as_deref());
    print_env("KITTY_WINDOW_ID", hints.kitty_window_id.as_deref());
    print_env("WEZTERM_EXECUTABLE", hints.wezterm_executable.as_deref());
    print_env("WT_SESSION", hints.windows_terminal_session.as_deref());

    println!("\nBackend   : {} ({})", detection.backend, detection.reason);
    println!(
        "Budget    : up to {} px tall at {} fps",
        budget.max_raster_height, budget.max_fps
    );
    println!(
        "Viewport  : {}x{} cells at {}x{} px/cell -> {}x{} raster",
        viewport.cols,
        viewport.rows,
        viewport.cell.width_px,
        viewport.cell.height_px,
        raster.width,
        raster.height
    );

    if let Some(multiplexer) = hints.multiplexer() {
        println!("Multiplexer: {multiplexer}");
        match multiplexer {
            Multiplexer::Tmux => println!(
                "             set `allow-passthrough on` for kitty graphics (docs/PROTOCOLS.md)"
            ),
            Multiplexer::Screen => println!(
                "             screen has no graphics passthrough; expect the ansi fallback"
            ),
        }
    }

    println!(
        "\nHint-based detection only: active probing (kitty graphics query, sixel DA,\ncell pixel size via CSI 16 t) lands in M2."
    );
    ExitCode::SUCCESS
}

/// Reads a local HLS master playlist, lists its variants and previews selection.
fn probe(args: Vec<String>) -> ExitCode {
    let mut path: Option<String> = None;
    let mut quality = Quality::Auto;
    let mut args = args.into_iter();

    while let Some(argument) = args.next() {
        if argument == "--quality" {
            let Some(value) = args.next() else {
                eprintln!("framefeed probe: --quality needs a value");
                return ExitCode::from(2);
            };
            match Quality::parse(&value) {
                Ok(parsed) => quality = parsed,
                Err(error) => {
                    eprintln!("framefeed probe: {error}");
                    return ExitCode::from(2);
                }
            }
        } else if let Some(value) = argument.strip_prefix("--quality=") {
            match Quality::parse(value) {
                Ok(parsed) => quality = parsed,
                Err(error) => {
                    eprintln!("framefeed probe: {error}");
                    return ExitCode::from(2);
                }
            }
        } else if argument.starts_with('-') {
            eprintln!("framefeed probe: unknown option `{argument}`");
            return ExitCode::from(2);
        } else if path.is_none() {
            path = Some(argument);
        } else {
            eprintln!("framefeed probe: unexpected argument `{argument}`");
            return ExitCode::from(2);
        }
    }

    let Some(path) = path else {
        eprintln!("usage: framefeed probe <playlist.m3u8> [--quality 360p]");
        return ExitCode::from(2);
    };

    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("framefeed probe: cannot read {path}: {error}");
            return ExitCode::from(1);
        }
    };
    let playlist = match parse_master_playlist(&text) {
        Ok(playlist) => playlist,
        Err(error) => {
            eprintln!("framefeed probe: {path}: {error}");
            return ExitCode::from(1);
        }
    };

    println!("{path}: {} variants", playlist.variants.len());
    for variant in &playlist.variants {
        println!(
            "  {:<11} {:>7} kbit/s  {}",
            describe_resolution(variant.resolution),
            variant.bandwidth / 1000,
            describe_codecs(variant)
        );
    }

    match quality {
        Quality::Auto => println!(
            "\nauto quality is resolved from the viewport at play time; pass --quality <height> to preview a choice"
        ),
        Quality::Height(height) => match select_variant(&playlist.variants, height) {
            Some(variant) => {
                println!(
                    "\nselected for {quality}: {} at {} kbit/s",
                    describe_resolution(variant.resolution),
                    variant.bandwidth / 1000
                );
                println!("  {}", resolve_uri(&path, &variant.uri));
            }
            None => println!("\nno variant declares a resolution; selection unavailable"),
        },
    }

    ExitCode::SUCCESS
}

fn describe_resolution(resolution: Option<framefeed_core::Size>) -> String {
    resolution.map_or_else(
        || "unknown".to_string(),
        |size| format!("{}x{}", size.width, size.height),
    )
}

fn describe_codecs(variant: &framefeed_source::Variant) -> String {
    if variant.codecs.is_empty() {
        "-".to_string()
    } else {
        variant.codecs.join(",")
    }
}

fn print_env(name: &str, value: Option<&str>) {
    match value {
        Some(value) => println!("  {name:<20} {value}"),
        None => println!("  {name:<20} (unset)"),
    }
}

/// Viewport from `COLUMNS`/`LINES`, falling back to 80x24.
///
/// The cell size is the conservative default until M2 adds `CSI 16 t` probing.
fn viewport_from_env() -> Viewport {
    let cols = env_dimension("COLUMNS").unwrap_or(80);
    let rows = env_dimension("LINES").unwrap_or(24);
    Viewport::new(cols, rows, DEFAULT_CELL).unwrap_or_else(|| {
        Viewport::new(80, 24, DEFAULT_CELL).expect("default viewport is non-zero")
    })
}

fn env_dimension(name: &str) -> Option<u16> {
    std::env::var(name)
        .ok()?
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|value| *value > 0)
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
