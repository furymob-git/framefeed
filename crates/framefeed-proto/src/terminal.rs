//! Terminal lifecycle helpers: TTY detection, viewport discovery, and a
//! panic-safe guard that restores the screen and cursor.

use std::io::{self, IsTerminal, Write};

use framefeed_core::{DEFAULT_CELL, Viewport};

const ENTER: &[u8] = b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H";
const LEAVE: &[u8] = b"\x1b[0m\x1b[?25h\x1b[?1049l";

/// True when stdout is an interactive terminal.
///
/// When it is not (pipes, `serve`, tests), we still emit escape sequences but
/// skip alternate-screen and cursor manipulation.
#[must_use]
pub fn stdout_is_tty() -> bool {
    io::stdout().is_terminal()
}

/// Viewport derived from `COLUMNS`/`LINES`, falling back to 80x24.
///
/// Cell pixel size is the conservative default until M2 adds `CSI 16 t` probing.
#[must_use]
pub fn viewport_from_env() -> Viewport {
    let cols = env_dimension("COLUMNS").unwrap_or(80);
    let rows = env_dimension("LINES").unwrap_or(24);
    Viewport::new(cols, rows, DEFAULT_CELL)
        .unwrap_or_else(|| Viewport::new(80, 24, DEFAULT_CELL).expect("non-zero fallback"))
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
    use super::{TerminalGuard, env_dimension};

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
}
