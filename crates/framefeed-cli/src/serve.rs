//! `framefeed serve`: terminal video over HTTP for `curl`.

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::ExitCode;

use framefeed_server::{Library, ServerConfig, run as run_server};

pub fn run(args: Vec<String>) -> ExitCode {
    let mut listen = "127.0.0.1:8080".to_string();
    let mut allow_local = false;
    let mut max_streams = 4usize;
    let mut media_dir: Option<String> = None;
    let mut args = args.into_iter();

    while let Some(argument) = args.next() {
        let (flag, inline) = match argument.split_once('=') {
            Some((flag, value)) => (flag, Some(value.to_string())),
            None => (argument.as_str(), None),
        };
        match flag {
            "--listen" | "-l" => match take_value(&mut args, inline, flag) {
                Ok(value) => listen = value,
                Err(error) => return fail(&error),
            },
            "--max-streams" => match take_value(&mut args, inline, flag) {
                Ok(value) => match value.trim().parse::<usize>() {
                    Ok(parsed) if parsed > 0 => max_streams = parsed,
                    _ => return fail("--max-streams expects a positive integer"),
                },
                Err(error) => return fail(&error),
            },
            "--media" | "-m" => match take_value(&mut args, inline, flag) {
                Ok(value) => media_dir = Some(value),
                Err(error) => return fail(&error),
            },
            "--allow-local" => allow_local = true,
            other if other.starts_with('-') => {
                return fail(&format!("unknown option `{other}`"));
            }
            other => return fail(&format!("unexpected argument `{other}`")),
        }
    }

    // Default to the curated demo library when run from a checkout.
    let media = media_dir.or_else(|| {
        let default = PathBuf::from("videos/demo");
        default.is_dir().then(|| default.display().to_string())
    });

    let listener = match TcpListener::bind(&listen) {
        Ok(listener) => listener,
        Err(error) => return fail(&format!("cannot bind {listen}: {error}")),
    };
    let address = listener
        .local_addr()
        .map(|address| address.to_string())
        .unwrap_or(listen);

    println!("framefeed serve listening on http://{address}");
    println!("  index: curl 'http://{address}/'");
    if let Some(dir) = &media {
        println!("  clips: curl -N 'http://{address}/<name>'   (library: {dir})");
    } else {
        println!("  no clip library: pass --media <dir> to serve /<name>");
    }
    println!("  try: curl -N 'http://{address}/stream?url=./clip.mp4&q=144p&backend=ansi'");
    println!("  health: curl 'http://{address}/healthz'");
    if !allow_local {
        println!("  local url= paths are disabled; pass --allow-local to serve files");
    }

    match run_server(
        listener,
        ServerConfig {
            allow_local,
            max_streams,
            media: media.map(Library::new),
        },
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => fail(&format!("server stopped: {error}")),
    }
}

fn take_value(
    args: &mut impl Iterator<Item = String>,
    inline: Option<String>,
    flag: &str,
) -> Result<String, String> {
    match inline {
        Some(value) => Ok(value),
        None => args.next().ok_or_else(|| format!("{flag} needs a value")),
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("framefeed serve: {message}");
    ExitCode::from(2)
}
