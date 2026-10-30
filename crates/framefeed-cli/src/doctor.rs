//! `framefeed doctor`: what the current terminal can do.

use std::process::ExitCode;

use framefeed_proto::{Hints, Multiplexer, detect, viewport_from_env};

pub fn run(_args: Vec<String>) -> ExitCode {
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
    println!("Backends  : kitty, sixel, ansi (iterm2 planned)");
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
    println!("Windows Terminal 1.22+ is detected as sixel: it draws real pixels, not half-blocks.");
    ExitCode::SUCCESS
}

fn print_env(name: &str, value: Option<&str>) {
    match value {
        Some(value) => println!("  {name:<20} {value}"),
        None => println!("  {name:<20} (unset)"),
    }
}
