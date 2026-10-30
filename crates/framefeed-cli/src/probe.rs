//! `framefeed probe`: inspect an HLS master playlist and preview `--quality`.

use std::process::ExitCode;

use framefeed_core::Size;
use framefeed_source::{Quality, Variant, parse_master_playlist, resolve_uri, select_variant};

pub fn run(args: Vec<String>) -> ExitCode {
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

fn describe_resolution(resolution: Option<Size>) -> String {
    resolution.map_or_else(
        || "unknown".to_string(),
        |size| format!("{}x{}", size.width, size.height),
    )
}

fn describe_codecs(variant: &Variant) -> String {
    if variant.codecs.is_empty() {
        "-".to_string()
    } else {
        variant.codecs.join(",")
    }
}
