//! Terminal backend detection.
//!
//! M1 ships *hint-based* detection only: environment variables that terminals set
//! for their own children. It is cheap, testable and covers the common cases.
//! Active probing (Kitty graphics query, Sixel DA, cell pixel size) lands in M2
//! and will refine this decision; see `docs/PROTOCOLS.md`.

use std::fmt;

use crate::budget::Budget;

/// Display backend framefeed can drive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    /// Kitty graphics protocol (kitty, WezTerm, Ghostty, Konsole, ...).
    Kitty,
    /// DEC Sixel graphics.
    Sixel,
    /// iTerm2 inline image protocol.
    Iterm2,
    /// Truecolor ANSI half-blocks; works nearly everywhere.
    Ansi,
}

impl Backend {
    /// Stable identifier used on the command line and in logs.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Kitty => "kitty",
            Self::Sixel => "sixel",
            Self::Iterm2 => "iterm2",
            Self::Ansi => "ansi",
        }
    }

    /// Performance envelope for this backend.
    #[must_use]
    pub const fn budget(self) -> Budget {
        match self {
            Self::Kitty => Budget::KITTY,
            Self::Sixel => Budget::SIXEL,
            Self::Iterm2 => Budget::ITERM2,
            Self::Ansi => Budget::ANSI,
        }
    }
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Multiplexer between us and the terminal, which changes how graphics travel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Multiplexer {
    /// tmux: needs `allow-passthrough on` for Kitty, or a Sixel-enabled build.
    Tmux,
    /// GNU screen: no graphics passthrough, expect the ANSI fallback.
    Screen,
}

impl Multiplexer {
    /// Stable identifier used in diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tmux => "tmux",
            Self::Screen => "screen",
        }
    }
}

impl fmt::Display for Multiplexer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Environment facts relevant to graphics support.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hints {
    /// `TERM`.
    pub term: Option<String>,
    /// `TERM_PROGRAM` (macOS and some cross-platform terminals).
    pub term_program: Option<String>,
    /// `KITTY_WINDOW_ID`, set by kitty for every child process.
    pub kitty_window_id: Option<String>,
    /// `WEZTERM_EXECUTABLE`, set by WezTerm.
    pub wezterm_executable: Option<String>,
    /// `WT_SESSION`, set by Windows Terminal.
    pub windows_terminal_session: Option<String>,
    /// `TMUX`, set inside tmux panes.
    pub tmux: Option<String>,
    /// `STY`, set inside GNU screen.
    pub screen: Option<String>,
}

impl Hints {
    /// Reads the hints from the current process environment.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            term: env_var("TERM"),
            term_program: env_var("TERM_PROGRAM"),
            kitty_window_id: env_var("KITTY_WINDOW_ID"),
            wezterm_executable: env_var("WEZTERM_EXECUTABLE"),
            windows_terminal_session: env_var("WT_SESSION"),
            tmux: env_var("TMUX"),
            screen: env_var("STY"),
        }
    }

    /// Multiplexer we are running under, if any.
    #[must_use]
    pub fn multiplexer(&self) -> Option<Multiplexer> {
        if self.tmux.is_some() {
            Some(Multiplexer::Tmux)
        } else if self.screen.is_some() {
            Some(Multiplexer::Screen)
        } else {
            None
        }
    }
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// A backend choice plus the reason it was chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Detection {
    /// Chosen backend.
    pub backend: Backend,
    /// Short human-readable justification, shown by `framefeed doctor`.
    pub reason: &'static str,
}

/// Picks a backend from environment hints.
///
/// Order matters: explicit terminal identity beats generic `TERM` values, and
/// any graphics protocol beats the ANSI fallback.
#[must_use]
pub fn detect(hints: &Hints) -> Detection {
    let term = hints
        .term
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let program = hints
        .term_program
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();

    let kitty = |reason| Detection {
        backend: Backend::Kitty,
        reason,
    };
    let sixel = |reason| Detection {
        backend: Backend::Sixel,
        reason,
    };
    let iterm2 = |reason| Detection {
        backend: Backend::Iterm2,
        reason,
    };
    let ansi = |reason| Detection {
        backend: Backend::Ansi,
        reason,
    };

    if hints.kitty_window_id.is_some() {
        return kitty("KITTY_WINDOW_ID is set");
    }
    if hints.wezterm_executable.is_some() {
        return kitty("WEZTERM_EXECUTABLE is set (requires enable_kitty_graphics = true)");
    }
    if term == "xterm-kitty" {
        return kitty("TERM is xterm-kitty");
    }
    if program.contains("wezterm") {
        return kitty("TERM_PROGRAM is WezTerm (requires enable_kitty_graphics = true)");
    }
    if program.contains("ghostty") || term.contains("ghostty") {
        return kitty("Ghostty detected");
    }
    if program.contains("iterm") {
        return iterm2("TERM_PROGRAM is iTerm2");
    }
    if hints.windows_terminal_session.is_some() {
        return sixel("WT_SESSION is set (Sixel requires Windows Terminal 1.22+)");
    }
    if term.contains("foot") {
        return sixel("TERM identifies foot");
    }
    if term.contains("alacritty") {
        return ansi("Alacritty ships no graphics protocol");
    }
    if program.contains("apple_terminal") {
        return ansi("Terminal.app has no graphics protocol");
    }
    ansi("no graphics protocol hint found")
}

#[cfg(test)]
mod tests {
    use super::{Backend, Hints, Multiplexer, detect};

    fn hints() -> Hints {
        Hints::default()
    }

    #[test]
    fn kitty_wins_over_everything_else() {
        let detection = detect(&Hints {
            term: Some("xterm-256color".into()),
            kitty_window_id: Some("1".into()),
            tmux: Some("/tmp/tmux".into()),
            ..hints()
        });
        assert_eq!(detection.backend, Backend::Kitty);
        assert!(detection.reason.contains("KITTY_WINDOW_ID"));
    }

    #[test]
    fn wezterm_uses_kitty_graphics() {
        let detection = detect(&Hints {
            wezterm_executable: Some("C:/Program Files/WezTerm/wezterm-gui.exe".into()),
            ..hints()
        });
        assert_eq!(detection.backend, Backend::Kitty);
    }

    #[test]
    fn windows_terminal_falls_back_to_sixel() {
        let detection = detect(&Hints {
            windows_terminal_session: Some("guid".into()),
            term_program: None,
            ..hints()
        });
        assert_eq!(detection.backend, Backend::Sixel);
    }

    #[test]
    fn alacritty_and_terminal_app_get_ansi() {
        let alacritty = detect(&Hints {
            term: Some("alacritty".into()),
            ..hints()
        });
        assert_eq!(alacritty.backend, Backend::Ansi);

        let apple = detect(&Hints {
            term_program: Some("Apple_Terminal".into()),
            ..hints()
        });
        assert_eq!(apple.backend, Backend::Ansi);
    }

    #[test]
    fn iterm_and_foot_are_recognised() {
        assert_eq!(
            detect(&Hints {
                term_program: Some("iTerm.app".into()),
                ..hints()
            })
            .backend,
            Backend::Iterm2
        );
        assert_eq!(
            detect(&Hints {
                term: Some("foot".into()),
                ..hints()
            })
            .backend,
            Backend::Sixel
        );
    }

    #[test]
    fn unknown_terminal_degrades_instead_of_failing() {
        assert_eq!(detect(&hints()).backend, Backend::Ansi);
    }

    #[test]
    fn multiplexer_prefers_tmux_over_screen() {
        let hints = Hints {
            tmux: Some("x".into()),
            screen: Some("y".into()),
            ..hints()
        };
        assert_eq!(hints.multiplexer(), Some(Multiplexer::Tmux));
        assert_eq!(Multiplexer::Tmux.name(), "tmux");
    }

    #[test]
    fn budgets_are_ordered_by_cost() {
        assert!(
            Backend::Kitty.budget().max_raster_height >= Backend::Sixel.budget().max_raster_height
        );
        assert!(Backend::Kitty.budget().max_fps >= Backend::Ansi.budget().max_fps);
    }
}
