//! Bundled clip library served at `GET /<name>`.
//!
//! The library is a flat directory of curated files (see `videos/demo/`). A request
//! path maps to a stem, never to a filesystem path: only `[A-Za-z0-9_-]` characters
//! are accepted, so `..`, `/` and `%2e%2e` cannot escape the directory.

use std::fs;
use std::path::{Path, PathBuf};

/// Extensions the library will serve, in resolution order.
pub const EXTENSIONS: [&str; 6] = ["mp4", "webm", "mkv", "mov", "m4v", "ogv"];

/// A directory of clips addressable by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Library {
    root: PathBuf,
}

impl Library {
    /// Wraps `root` as a library. The directory may not exist yet.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Directory backing the library.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves a request path such as `/big-buck-bunny` or `/big-buck-bunny.mp4`.
    ///
    /// Returns `None` for anything that is not a plain name resolving to an existing
    /// file, so traversal attempts and unknown clips are indistinguishable to clients.
    #[must_use]
    pub fn resolve(&self, request_path: &str) -> Option<PathBuf> {
        let name = request_path.trim_matches('/');
        let stem = strip_known_extension(name);
        if stem.is_empty() || !is_plain_name(stem) {
            return None;
        }
        EXTENSIONS
            .iter()
            .map(|extension| self.root.join(format!("{stem}.{extension}")))
            .find(|candidate| candidate.is_file())
    }

    /// Sorted clip names available on disk, without extensions.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                if !path.is_file() {
                    return None;
                }
                let extension = path.extension()?.to_str()?.to_ascii_lowercase();
                if !EXTENSIONS.contains(&extension.as_str()) {
                    return None;
                }
                let stem = path.file_stem()?.to_str()?.to_string();
                is_plain_name(&stem).then_some(stem)
            })
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

fn strip_known_extension(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, extension))
            if EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()) =>
        {
            stem
        }
        _ => name,
    }
}

fn is_plain_name(name: &str) -> bool {
    name.chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
}

#[cfg(test)]
mod tests {
    use super::Library;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("framefeed-{name}-{unique}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn resolves_stems_with_and_without_extension() {
        let root = scratch("media");
        fs::write(root.join("clip.mp4"), b"x").unwrap();
        fs::write(root.join("other.webm"), b"x").unwrap();
        let library = Library::new(&root);

        assert_eq!(library.resolve("/clip"), Some(root.join("clip.mp4")));
        assert_eq!(library.resolve("clip.mp4"), Some(root.join("clip.mp4")));
        assert_eq!(library.resolve("/other"), Some(root.join("other.webm")));
        assert_eq!(
            library.names(),
            vec!["clip".to_string(), "other".to_string()]
        );

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_traversal_and_unknown_names() {
        let root = scratch("guard");
        fs::write(root.join("clip.mp4"), b"x").unwrap();
        fs::write(root.join("notes.txt"), b"x").unwrap();
        let library = Library::new(&root);

        for attempt in [
            "/../clip",
            "/..%2fclip",
            "/clip/../clip",
            "/./clip",
            "/",
            "",
            "/missing",
            "/notes",
            "/clip.mp4.exe",
            "/clip;rm",
        ] {
            assert_eq!(library.resolve(attempt), None, "{attempt}");
        }

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn missing_directory_lists_nothing() {
        let library = Library::new(std::env::temp_dir().join("framefeed-does-not-exist"));
        assert!(library.names().is_empty());
        assert_eq!(library.resolve("/clip"), None);
    }
}
