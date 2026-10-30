//! Terminal lifecycle helpers: TTY detection, viewport discovery, and a
//! panic-safe guard that restores the screen and cursor.

use std::io::{self, IsTerminal, Write};

use framefeed_core::{CellSize, DEFAULT_CELL, Viewport};

use crate::detect::{Backend, Hints};

const ENTER: &[u8] = b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H";
const LEAVE: &[u8] = b"\x1b[0m\x1b[?25h\x1b[?1049l";

/// Cell geometry Windows Terminal uses for Sixel.
///
/// Windows Terminal stores Sixel images on a fixed 10×20 pixel grid and then
/// stretches each grid cell to the font's cell size. Planning a raster in 8×16
/// cells therefore gets resampled twice (once by us, once by the terminal) and
/// comes out blocky; matching 10×20 keeps a single unavoidable scale step.
pub const WT_SIXEL_CELL: CellSize = CellSize {
    width_px: 10,
    height_px: 20,
};

/// True when stdout is an interactive terminal.
///
/// When it is not (pipes, `serve`, tests), we still emit escape sequences but
/// skip alternate-screen and cursor manipulation.
#[must_use]
pub fn stdout_is_tty() -> bool {
    io::stdout().is_terminal()
}

/// Viewport for the current terminal, in cells and pixels.
///
/// Uses the real terminal size when there is one, then `COLUMNS`/`LINES`, then
/// the conservative 80×24 fallback.
#[must_use]
pub fn viewport() -> Viewport {
    let (cols, rows) = terminal_dimensions().unwrap_or((80, 24));
    Viewport::new(cols, rows, DEFAULT_CELL)
        .unwrap_or_else(|| Viewport::new(80, 24, DEFAULT_CELL).expect("non-zero fallback"))
}

/// Viewport using the cell geometry the chosen backend actually draws with.
///
/// Only Windows Terminal's Sixel path differs today; every other backend maps
/// one raster pixel to one device pixel and is fine with [`DEFAULT_CELL`].
#[must_use]
pub fn viewport_for(backend: Backend) -> Viewport {
    let cell = if backend == Backend::Sixel && Hints::from_env().windows_terminal_session.is_some()
    {
        WT_SIXEL_CELL
    } else {
        DEFAULT_CELL
    };
    let (cols, rows) = terminal_dimensions().unwrap_or((80, 24));
    Viewport::new(cols, rows, cell)
        .unwrap_or_else(|| Viewport::new(80, 24, cell).expect("non-zero fallback"))
}

/// Backwards-compatible alias for [`viewport`].
#[must_use]
pub fn viewport_from_env() -> Viewport {
    viewport()
}

fn terminal_dimensions() -> Option<(u16, u16)> {
    if let Some((terminal_size::Width(width), terminal_size::Height(height))) =
        terminal_size::terminal_size()
    {
        if width > 0 && height > 0 {
            return Some((width, height));
        }
    }
    let cols = env_dimension("COLUMNS")?;
    let rows = env_dimension("LINES")?;
    Some((cols, rows))
}

fn env_dimension(name: &str) -> Option<u16> {
    std::env::var(name)
        .ok()?
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|value| *value > 0)
}

/// Owns the alternate screen, the hidden cursor and the reset state.
///
/// `restore` runs on `Drop`, so normal exit and `?` paths clean up. The release
/// profile must unwind for this to also cover panics (`panic = "unwind"`).
#[derive(Debug)]
pub struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    /// Enters the alternate screen if stdout is a terminal.
    #[must_use]
    pub fn enter() -> Self {
        if !stdout_is_tty() {
            return Self { active: false };
        }
        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(ENTER);
        let _ = stdout.flush();
        Self { active: true }
    }

    /// Whether the guard actually changed terminal state.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Restores the cursor and leaves the alternate screen. Idempotent.
    pub fn restore(&mut self) {
        if !self.active {
            return;
        }
        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(LEAVE);
        let _ = stdout.flush();
        self.active = false;
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::{TerminalGuard, WT_SIXEL_CELL, env_dimension, viewport, viewport_for};
    use crate::detect::Backend;

    #[test]
    fn guard_is_inert_without_a_tty() {
        // Tests never run on a terminal, so this exercises the inert path.
        let mut guard = TerminalGuard::enter();
        assert!(!guard.is_active() || super::stdout_is_tty());
        guard.restore();
        guard.restore(); // idempotent
    }

    #[test]
    fn env_dimensions_reject_garbage() {
        assert_eq!(env_dimension("FRAMEFEED_DOES_NOT_EXIST"), None);
    }

    #[test]
    fn viewports_are_never_empty() {
        assert!(viewport().cols > 0 && viewport().rows > 0);
        assert!(viewport_for(Backend::Ansi).cols > 0);
    }

    #[test]
    fn windows_terminal_sixel_uses_the_10x20_grid() {
        assert_eq!(WT_SIXEL_CELL.width_px, 10);
        assert_eq!(WT_SIXEL_CELL.height_px, 20);
    }
}
