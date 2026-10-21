//! App preferences, stored as JSON in the app's config folder on this computer.
//!
//! Nothing secret goes here: the Stremio auth key and debrid keys live in the Credential Manager.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ranking::filters::StreamFilters;
use crate::ranking::score::ScoringRules;

const FILE_NAME: &str = "settings.json";

/// The largest profile picture accepted, as a data URL. The UI shrinks pictures well below this.
pub const MAX_AVATAR_LEN: usize = 512 * 1024;

/// How subtitles look. Styled subtitles (ASS, common for anime) keep their own look apart from
/// size and position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SubtitleStyle {
    /// Size relative to mpv's default, 0.5 to 2.0.
    pub scale: f64,
    /// Text colour as `#RRGGBB`.
    pub color: String,
    /// `"outline"` for outlined text, `"box"` for text on a dark box.
    pub background: String,
    /// Vertical position in percent of the screen height (100 is the bottom edge).
    pub position: u8,
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        Self {
            scale: 1.0,
            color: "#FFFFFF".to_string(),
            background: "outline".to_string(),
            position: 100,
        }
    }
}

impl SubtitleStyle {
    pub const MIN_SCALE: f64 = 0.5;
    pub const MAX_SCALE: f64 = 2.0;

    fn validate(&self) -> Result<(), String> {
        if !(Self::MIN_SCALE..=Self::MAX_SCALE).contains(&self.scale) {
            return Err("subtitle size must be between 50% and 200%".into());
        }
        if !is_hex_color(&self.color) {
            return Err("subtitle colour must look like #RRGGBB".into());
        }
        if self.background != "outline" && self.background != "box" {
            return Err("subtitle background must be outline or box".into());
        }
        if !(50..=100).contains(&self.position) {
            return Err("subtitle position must be between 50 and 100".into());
        }
        Ok(())
    }

    /// The mpv properties that apply this style.
    pub fn mpv_properties(&self) -> Vec<(&'static str, String)> {
        let border = if self.background == "box" {
            "background-box"
        } else {
            "outline-and-shadow"
        };
        vec![
            ("sub-scale", format!("{:.2}", self.scale)),
            ("sub-color", self.color.clone()),
            ("sub-border-style", border.to_string()),
            ("sub-back-color", "#B0000000".to_string()),
            ("sub-pos", self.position.to_string()),
        ]
    }
}

/// Whether `text` is a colour written as `#RRGGBB`.
fn is_hex_color(text: &str) -> bool {
    let hex = text.strip_prefix('#').unwrap_or_default();
    hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// How the app looks: the light or dark mode and the colour its Material You palette is built
/// from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Appearance {
    /// `"system"` follows Windows' light or dark setting; `"light"` or `"dark"` fix it.
    pub mode: String,
    /// The accent colour as `#RRGGBB`.
    pub accent: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: "system".to_string(),
            accent: "#7C8CFF".to_string(),
        }
    }
}

impl Appearance {
    fn validate(&self) -> Result<(), String> {
        if !["system", "light", "dark"].contains(&self.mode.as_str()) {
            return Err("the theme must be system, light or dark".into());
        }
        if !is_hex_color(&self.accent) {
            return Err("the accent colour must look like #RRGGBB".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Audio languages in order of preference (ISO 639-1).
    pub audio_languages: Vec<String>,
    /// Subtitle languages in order of preference (ISO 639-1), used once subtitles arrive.
    pub subtitle_languages: Vec<String>,
    /// Highest resolution worth preferring when ranking.
    pub preferred_resolution: u16,
    pub filters: StreamFilters,
    /// Profile picture as a `data:image/...` URL, kept only on this computer.
    pub avatar: Option<String>,
    /// Let the torrent engine use DHT to find peers. Its UDP socket is the only thing in
    /// Spaceport that listens on the network; off means trackers only.
    pub torrent_dht: bool,
    /// Most gigabytes downloaded torrents may use; torrents not being watched are deleted,
    /// least recently watched first, beyond it. Everything is deleted when Spaceport exits.
    pub torrent_cache_gb: u32,
    pub subtitle_style: SubtitleStyle,
    /// Skip anime intros, outros and recaps automatically (AniSkip times).
    pub anime_auto_skip: bool,
    pub appearance: Appearance,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            audio_languages: vec!["en".to_string()],
            subtitle_languages: vec!["en".to_string()],
            preferred_resolution: 2160,
            filters: StreamFilters::default(),
            avatar: None,
            torrent_dht: true,
            torrent_cache_gb: 10,
            subtitle_style: SubtitleStyle::default(),
            anime_auto_skip: false,
            appearance: Appearance::default(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("could not save settings: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not save settings: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

impl Settings {
    /// Scoring rules with the user's preferences applied over the defaults.
    pub fn scoring_rules(&self) -> ScoringRules {
        ScoringRules {
            preferred_resolution: self.preferred_resolution,
            preferred_languages: self.audio_languages.clone(),
            ..ScoringRules::default()
        }
    }

    /// Rejects values the UI should never send, so a bad file cannot break ranking.
    pub fn validate(&self) -> Result<(), SettingsError> {
        let valid_code = |c: &String| c.len() == 2 && c.chars().all(|ch| ch.is_ascii_lowercase());
        if !self.audio_languages.iter().all(valid_code)
            || !self.subtitle_languages.iter().all(valid_code)
        {
            return Err(SettingsError::Invalid(
                "languages must be two-letter codes".into(),
            ));
        }
        self.subtitle_style
            .validate()
            .map_err(SettingsError::Invalid)?;
        self.appearance.validate().map_err(SettingsError::Invalid)?;
        if !(1..=2000).contains(&self.torrent_cache_gb) {
            return Err(SettingsError::Invalid(
                "the torrent cache must be between 1 and 2000 GB".into(),
            ));
        }
        if let Some(avatar) = &self.avatar {
            if !avatar.starts_with("data:image/") || avatar.len() > MAX_AVATAR_LEN {
                return Err(SettingsError::Invalid(
                    "the profile picture must be a small image".into(),
                ));
            }
        }
        Ok(())
    }
}

/// Reads and writes the settings file.
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join(FILE_NAME),
        }
    }

    /// Loads the saved settings. A missing or unreadable file falls back to the defaults.
    pub fn load(&self) -> Settings {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
            .filter(|s| s.validate().is_ok())
            .unwrap_or_default()
    }

    /// Writes to a temporary file first so a crash cannot leave a half-written file.
    pub fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        settings.validate()?;
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("spaceport-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn checks_the_appearance() {
        let mut settings = Settings::default();
        assert_eq!(settings.appearance.mode, "system");
        settings.appearance.mode = "dark".into();
        settings.appearance.accent = "#1a73e8".into();
        assert!(settings.validate().is_ok());
        settings.appearance.mode = "dim".into();
        assert!(settings.validate().is_err());
        settings.appearance.mode = "light".into();
        settings.appearance.accent = "blue".into();
        assert!(settings.validate().is_err());
    }

    #[test]
    fn round_trips_and_defaults() {
        let dir = temp_dir("roundtrip");
        let store = SettingsStore::new(&dir);
        assert_eq!(store.load(), Settings::default());
        let settings = Settings {
            audio_languages: vec!["de".into(), "en".into()],
            preferred_resolution: 1080,
            filters: StreamFilters {
                max_size: Some(20),
                min_resolution: Some(720),
                blocked_codecs: vec!["AV1".into()],
            },
            avatar: Some("data:image/png;base64,AAAA".into()),
            ..Settings::default()
        };
        store.save(&settings).unwrap();
        assert_eq!(store.load(), settings);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn broken_file_falls_back_to_defaults() {
        let dir = temp_dir("broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(FILE_NAME), "{not json").unwrap();
        assert_eq!(SettingsStore::new(&dir).load(), Settings::default());
        // Missing fields take their defaults.
        fs::write(dir.join(FILE_NAME), r#"{"preferredResolution":720}"#).unwrap();
        let loaded = SettingsStore::new(&dir).load();
        assert_eq!(loaded.preferred_resolution, 720);
        assert_eq!(loaded.audio_languages, ["en"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_bad_values() {
        let bad_language = Settings {
            audio_languages: vec!["English".into()],
            ..Settings::default()
        };
        assert!(bad_language.validate().is_err());
        let bad_avatar = Settings {
            avatar: Some("https://tracker.example/pixel.png".into()),
            ..Settings::default()
        };
        assert!(bad_avatar.validate().is_err());
    }

    #[test]
    fn checks_subtitle_style() {
        let mut settings = Settings::default();
        assert!(settings.validate().is_ok());
        settings.subtitle_style.scale = 3.0;
        assert!(settings.validate().is_err());
        settings.subtitle_style = SubtitleStyle {
            color: "yellow".into(),
            ..SubtitleStyle::default()
        };
        assert!(settings.validate().is_err());
        settings.subtitle_style = SubtitleStyle {
            background: "glow".into(),
            ..SubtitleStyle::default()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn subtitle_style_maps_to_mpv() {
        let style = SubtitleStyle {
            scale: 1.25,
            color: "#FFFF00".into(),
            background: "box".into(),
            position: 95,
        };
        let props = style.mpv_properties();
        assert!(props.contains(&("sub-scale", "1.25".to_string())));
        assert!(props.contains(&("sub-color", "#FFFF00".to_string())));
        assert!(props.contains(&("sub-border-style", "background-box".to_string())));
        assert!(props.contains(&("sub-pos", "95".to_string())));
    }

    #[test]
    fn preferences_feed_scoring() {
        let settings = Settings {
            audio_languages: vec!["de".into()],
            preferred_resolution: 1080,
            ..Settings::default()
        };
        let rules = settings.scoring_rules();
        assert_eq!(rules.preferred_languages, ["de"]);
        assert_eq!(rules.preferred_resolution, 1080);
    }
}
