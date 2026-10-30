//! `framefeed play`: decode a source and paint it into the terminal.

use std::io::Write;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use framefeed_core::{Fit, Pace, Pacer, Raster, Size, plan_render};
use framefeed_decode::{Decoder, FfmpegDecoder, OutputSize, probe};
use framefeed_proto::{
    AnsiEncoder, Backend, Hints, KittyEncoder, KittyOptions, TerminalGuard, detect,
    viewport_from_env,
};
use framefeed_source::Quality;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackendChoice {
    Auto,
    Kitty,
    Ansi,
}

impl BackendChoice {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "kitty" => Ok(Self::Kitty),
            "ansi" => Ok(Self::Ansi),
            other => Err(format!(
                "unknown backend `{other}`; use auto, kitty or ansi (sixel/iterm2 are not implemented yet)"
            )),
        }
    }
}

#[derive(Debug)]
struct PlayOptions {
    source: String,
    quality: Quality,
    backend: BackendChoice,
    fps: Option<u8>,
    fit: Fit,
    stats: bool,
}

enum Encoder {
    Ansi(AnsiEncoder),
    Kitty(KittyEncoder),
}

impl Encoder {
    fn encode(
        &mut self,
        raster: &Raster,
        cols: u16,
        rows: u16,
        out: &mut Vec<u8>,
    ) -> Result<(), String> {
        match self {
            Self::Ansi(encoder) => {
                encoder.encode_frame(raster, out);
                Ok(())
            }
            Self::Kitty(encoder) => encoder
                .encode_frame(raster, cols, rows, out)
                .map_err(|error| error.to_string()),
        }
    }
}

pub fn run(args: Vec<String>) -> ExitCode {
    let options = match parse(args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("framefeed play: {error}");
            return ExitCode::from(2);
        }
    };
    match play(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("framefeed play: {error}");
            ExitCode::from(1)
        }
    }
}

fn play(options: PlayOptions) -> Result<(), String> {
    let viewport = viewport_from_env();
    let backend = resolve_backend(options.backend);
    let budget = backend.budget();

    let metadata = probe(&options.source).map_err(|error| error.to_string())?;
    let plan = plan_render(
        viewport,
        Size::new(metadata.width, metadata.height),
        options.fit,
        Some(
            options
                .quality
                .target_height(viewport.pixel_size().height)
                .min(budget.max_raster_height),
        ),
    );
    let output = OutputSize {
        width: plan.raster.width,
        height: plan.raster.height,
        crop: options.fit == Fit::Cover,
    };
    let mut decoder =
        FfmpegDecoder::open(&options.source, Some(output)).map_err(|error| error.to_string())?;

    let cols = cells(plan.raster.width, viewport.cell.width_px, viewport.cols);
    let rows = cells(plan.raster.height, viewport.cell.height_px, viewport.rows);

    let source_fps = decoder.metadata().fps.min(f64::from(budget.max_fps));
    let fps = options
        .fps
        .map_or(source_fps, |cap| source_fps.min(f64::from(cap)))
        .max(1.0);
    let mut pacer = Pacer::new(fps, Instant::now());

    let mut encoder = match backend {
        Backend::Kitty => Encoder::Kitty(
            KittyEncoder::new(KittyOptions::default()).map_err(|error| error.to_string())?,
        ),
        _ => Encoder::Ansi(AnsiEncoder::new()),
    };

    let stop = Arc::new(AtomicBool::new(false));
    let handler_flag = Arc::clone(&stop);
    let _ = ctrlc::set_handler(move || handler_flag.store(true, Ordering::SeqCst));

    let mut guard = TerminalGuard::enter();
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    let mut buffer = Vec::new();
    let mut frames = 0u64;
    let mut bytes = 0u64;
    let started = Instant::now();

    let result = loop {
        if stop.load(Ordering::Relaxed) {
            break Ok(());
        }
        let frame = match decoder.next_frame() {
            Ok(Some(frame)) => frame,
            Ok(None) => break Ok(()),
            Err(error) => break Err(error.to_string()),
        };
        if let Pace::Wait(delay) = pacer.tick(Instant::now()) {
            std::thread::sleep(delay);
        }
        buffer.clear();
        if let Err(error) = encoder.encode(frame, cols, rows, &mut buffer) {
            break Err(error);
        }
        if let Err(error) = writer.write_all(&buffer) {
            break Err(format!("writing to the terminal failed: {error}"));
        }
        let _ = writer.flush();
        frames += 1;
        bytes += u64::try_from(buffer.len()).unwrap_or_default();
    };

    if let Encoder::Kitty(encoder) = &encoder {
        buffer.clear();
        encoder.delete(&mut buffer);
        let _ = writer.write_all(&buffer);
        let _ = writer.flush();
    }
    drop(writer);
    guard.restore();

    if options.stats {
        let elapsed = started.elapsed().as_secs_f64().max(f64::EPSILON);
        eprintln!(
            "framefeed: backend={} raster={}x{} fps={fps:.2} frames={frames} dropped={} \
             {:.1} fps {:.1} MiB written ({:.0} KiB/frame) in {:.2}s",
            backend.name(),
            plan.raster.width,
            plan.raster.height,
            pacer.dropped(),
            frames as f64 / elapsed,
            bytes as f64 / (1024.0 * 1024.0),
            bytes as f64 / (1024.0 * frames.max(1) as f64),
            elapsed
        );
    }

    result
}

fn resolve_backend(choice: BackendChoice) -> Backend {
    match choice {
        BackendChoice::Kitty => Backend::Kitty,
        BackendChoice::Ansi => Backend::Ansi,
        BackendChoice::Auto => match detect(&Hints::from_env()).backend {
            Backend::Kitty => Backend::Kitty,
            other => {
                if other != Backend::Ansi {
                    eprintln!(
                        "framefeed play: {other} detected but not implemented yet; using ansi"
                    );
                }
                Backend::Ansi
            }
        },
    }
}

fn cells(pixels: u32, cell_px: u16, fallback: u16) -> u16 {
    let cell_px = u32::from(cell_px);
    if cell_px == 0 {
        return fallback;
    }
    u16::try_from(pixels.div_ceil(cell_px)).unwrap_or(fallback)
}

fn parse(args: Vec<String>) -> Result<PlayOptions, String> {
    let mut source = None;
    let mut quality = Quality::Auto;
    let mut backend = BackendChoice::Auto;
    let mut fps = None;
    let mut fit = Fit::Contain;
    let mut stats = false;
    let mut args = args.into_iter();

    while let Some(argument) = args.next() {
        let (flag, inline) = match argument.split_once('=') {
            Some((flag, value)) => (flag, Some(value.to_string())),
            None => (argument.as_str(), None),
        };
        match flag {
            "--quality" | "-q" => {
                let value = take_value(&mut args, inline, flag)?;
                quality = Quality::parse(&value).map_err(|error| error.to_string())?;
            }
            "--backend" => {
                let value = take_value(&mut args, inline, flag)?;
                backend = BackendChoice::parse(&value)?;
            }
            "--fps" => {
                let value = take_value(&mut args, inline, flag)?;
                let value = parse_number::<u8>(&value, "--fps")?;
                if value == 0 {
                    return Err("--fps must be at least 1".to_string());
                }
                fps = Some(value);
            }
            "--fit" => {
                let value = take_value(&mut args, inline, flag)?;
                fit = match value.as_str() {
                    "contain" => Fit::Contain,
                    "cover" => Fit::Cover,
                    "stretch" => Fit::Stretch,
                    other => {
                        return Err(format!(
                            "unknown fit `{other}`; use contain, cover or stretch"
                        ));
                    }
                };
            }
            "--stats" => stats = true,
            other if other.starts_with('-') => {
                return Err(format!("unknown option `{other}`"));
            }
            other => {
                if source.is_some() {
                    return Err(format!("unexpected argument `{other}`"));
                }
                source = Some(other.to_string());
            }
        }
    }

    Ok(PlayOptions {
        source: source.ok_or("missing <SOURCE> (see --help)")?,
        quality,
        backend,
        fps,
        fit,
        stats,
    })
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

fn parse_number<T: std::str::FromStr>(value: &str, flag: &str) -> Result<T, String> {
    value
        .trim()
        .parse()
        .map_err(|_| format!("{flag} expects a number, got `{value}`"))
}

#[cfg(test)]
mod tests {
    use super::{BackendChoice, Fit, Quality, parse};
    use framefeed_core::Viewport;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn parses_flags_in_both_forms() {
        let options = parse(args(&[
            "./clip.mp4",
            "--quality=360p",
            "--backend",
            "kitty",
            "--fps",
            "24",
            "--fit=cover",
            "--stats",
        ]))
        .unwrap();
        assert_eq!(options.source, "./clip.mp4");
        assert_eq!(options.quality, Quality::Height(360));
        assert_eq!(options.backend, BackendChoice::Kitty);
        assert_eq!(options.fps, Some(24));
        assert_eq!(options.fit, Fit::Cover);
        assert!(options.stats);
    }

    #[test]
    fn defaults_are_sane() {
        let options = parse(args(&["./clip.mp4"])).unwrap();
        assert_eq!(options.quality, Quality::Auto);
        assert_eq!(options.backend, BackendChoice::Auto);
        assert_eq!(options.fps, None);
        assert_eq!(options.fit, Fit::Contain);
        assert!(!options.stats);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(args(&[])).is_err());
        assert!(parse(args(&["./a", "./b"])).is_err());
        assert!(parse(args(&["./a", "--nope"])).is_err());
        assert!(parse(args(&["./a", "--fps"])).is_err());
        assert!(parse(args(&["./a", "--fps=0"])).is_err());
        assert!(parse(args(&["./a", "--backend=sixel"])).is_err());
        assert!(parse(args(&["./a", "--quality=hd"])).is_err());
    }

    #[test]
    fn cell_counts_follow_geometry() {
        let viewport = Viewport::new(80, 24, framefeed_core::DEFAULT_CELL).unwrap();
        assert_eq!(super::cells(640, viewport.cell.width_px, viewport.cols), 80);
        assert_eq!(
            super::cells(360, viewport.cell.height_px, viewport.rows),
            23
        );
    }
}
