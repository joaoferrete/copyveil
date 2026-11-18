//! The settings file: `$XDG_CONFIG_HOME/copyveil/settings.toml`.
//!
//! The daemon is the only owner of the settings: the app changes them over
//! D-Bus (`Settings1`), and the daemon validates and saves them here. A file
//! that can't be read, or holds values outside the limits in
//! `copyveil_core::settings`, falls back to the defaults with a warning; the
//! daemon never refuses to start over it.

use copyveil_core::Settings;
use copyveil_core::settings::{MAX_PLAIN_ITEMS, SECRET_EXPIRY, SUSPECTED_EXPIRY};
use copyveil_detect::{Detector, DetectorSet};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Everything the user can set, as stored.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Config {
    /// What shapes the History.
    pub settings: Settings,
    /// Keep the History in memory only, never on disk.
    pub memory_only: bool,
}

/// The file's layout. Durations are whole seconds; detectors are listed by
/// the ones turned off, so a detector added later is on by default.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    secret_expiry_seconds: u64,
    suspected_expiry_seconds: u64,
    max_plain_items: usize,
    #[serde(default)]
    disabled_detectors: Vec<String>,
    #[serde(default)]
    memory_only: bool,
}

impl Config {
    /// Where the file lives, or `None` if neither `XDG_CONFIG_HOME` nor `HOME` is set.
    #[must_use]
    pub fn path() -> Option<PathBuf> {
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
        Some(config_home.join("copyveil").join("settings.toml"))
    }

    /// Reads the file. A missing file means the defaults; an unreadable or
    /// out-of-limits one too, with a warning.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        Self::parse(&text).unwrap_or_else(|| {
            tracing::warn!("the settings file is invalid; using the defaults");
            Self::default()
        })
    }

    /// Parses the file's text, or `None` if it's malformed or out of limits.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let file: File = toml::from_str(text).ok()?;
        let mut detectors = DetectorSet::all();
        for name in &file.disabled_detectors {
            // An unknown name (a detector removed in a later version) is ignored.
            if let Some(detector) = Detector::from_name(name) {
                detectors = detectors.without(detector);
            }
        }
        let settings = Settings {
            secret_expiry: Duration::from_secs(file.secret_expiry_seconds),
            suspected_expiry: Duration::from_secs(file.suspected_expiry_seconds),
            max_plain_items: file.max_plain_items,
            detectors,
        };
        settings.validate().ok()?;
        Some(Self {
            settings,
            memory_only: file.memory_only,
        })
    }

    /// The file's text.
    #[must_use]
    pub fn render(&self) -> String {
        let file = File {
            secret_expiry_seconds: self.settings.secret_expiry.as_secs(),
            suspected_expiry_seconds: self.settings.suspected_expiry.as_secs(),
            max_plain_items: self.settings.max_plain_items,
            disabled_detectors: Detector::ALL
                .into_iter()
                .filter(|detector| !self.settings.detectors.contains(*detector))
                .map(|detector| detector.name().to_owned())
                .collect(),
            memory_only: self.memory_only,
        };
        let body = toml::to_string(&file).unwrap_or_default();
        format!(
            "# Copyveil settings. Change them in the app; the daemon checks every value.\n\
             # Limits: secret_expiry_seconds {}–{}, suspected_expiry_seconds {}–{}, \
             max_plain_items {}–{}.\n{body}",
            SECRET_EXPIRY.start().as_secs(),
            SECRET_EXPIRY.end().as_secs(),
            SUSPECTED_EXPIRY.start().as_secs(),
            SUSPECTED_EXPIRY.end().as_secs(),
            MAX_PLAIN_ITEMS.start(),
            MAX_PLAIN_ITEMS.end(),
        )
    }

    /// Writes the file atomically (a temporary file renamed into place).
    ///
    /// # Errors
    ///
    /// The directory or the file couldn't be written.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, self.render())?;
        std::fs::rename(&temporary, path)
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use copyveil_core::Settings;
    use copyveil_detect::{Detector, DetectorSet};
    use std::time::Duration;

    fn custom() -> Config {
        Config {
            settings: Settings {
                secret_expiry: Duration::from_secs(45),
                suspected_expiry: Duration::from_mins(20),
                max_plain_items: 1000,
                detectors: DetectorSet::all()
                    .without(Detector::HighEntropy)
                    .without(Detector::PaymentCard),
            },
            memory_only: true,
        }
    }

    #[test]
    fn a_config_round_trips_through_its_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("copyveil").join("settings.toml");
        custom().save(&path).unwrap();
        assert_eq!(Config::load(&path), custom());
        assert!(!path.with_extension("toml.new").exists());
    }

    #[test]
    fn the_file_lists_disabled_detectors_by_name() {
        let text = custom().render();
        assert!(text.contains("disabled_detectors = [\"payment-card\", \"high-entropy\"]"));
        assert!(text.contains("secret_expiry_seconds = 45"));
        assert!(text.contains("10–300"));
    }

    #[test]
    fn a_missing_file_means_the_defaults() {
        let dir = tempfile::TempDir::new().unwrap();
        assert_eq!(
            Config::load(&dir.path().join("none.toml")),
            Config::default()
        );
    }

    #[test]
    fn invalid_files_fall_back_to_the_defaults() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("settings.toml");
        for text in [
            "not toml at all [",
            "secret_expiry_seconds = 1\nsuspected_expiry_seconds = 900\nmax_plain_items = 500",
            "secret_expiry_seconds = 30\nsuspected_expiry_seconds = 900\nmax_plain_items = 500\nsurprise = 1",
            "secret_expiry_seconds = 30",
        ] {
            std::fs::write(&path, text).unwrap();
            assert_eq!(Config::load(&path), Config::default(), "{text}");
        }
    }

    #[test]
    fn unknown_detector_names_are_ignored() {
        let config = Config::parse(
            "secret_expiry_seconds = 30\nsuspected_expiry_seconds = 900\n\
             max_plain_items = 500\ndisabled_detectors = [\"jwt\", \"retired\"]",
        )
        .unwrap();
        assert_eq!(
            config.settings.detectors,
            DetectorSet::all().without(Detector::Jwt)
        );
        assert!(!config.memory_only);
    }
}
