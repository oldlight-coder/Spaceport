//! The playback state the controls window shows, built from mpv property changes.

use serde::{Deserialize, Serialize};

use super::mpv::PropertyValue;
use crate::language;
use crate::stremio::subtitles::AddonSubtitle;

/// An audio or subtitle track inside the playing file (or a subtitle added from an addon).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: i64,
    /// A readable name: language, title and format details.
    pub label: String,
    pub selected: bool,
    /// Loaded from outside the file (an addon subtitle); the UI lists those separately.
    pub external: bool,
    /// For subtitles loaded from a URL, that URL.
    #[serde(skip)]
    pub external_url: Option<String>,
}

/// A subtitle offered by an addon, as the controls list it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddonSubtitleView {
    pub key: String,
    pub label: String,
    pub addon_name: String,
    pub selected: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackState {
    pub title: String,
    /// Label of the stream being played, e.g. "[TB+] Torrentio 4k".
    pub stream: String,
    pub paused: bool,
    /// Waiting for the network to fill the cache.
    pub buffering: bool,
    pub position: f64,
    pub duration: f64,
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    /// mpv is moving to a new position.
    pub seeking: bool,
    /// Whether the stream allows seeking beyond what has loaded.
    pub seekable: bool,
    pub audio_tracks: Vec<Track>,
    /// Subtitle tracks mpv has, including addon subtitles once loaded (marked `external`).
    pub subtitle_tracks: Vec<Track>,
    pub addon_subtitles: Vec<AddonSubtitleView>,
    /// Every addon subtitle with its URL, kept for loading on demand.
    #[serde(skip)]
    pub addon_subtitle_files: Vec<AddonSubtitle>,
    /// Labels of the streams that can be switched to, best first.
    pub sources: Vec<String>,
    pub source_index: usize,
}

/// One entry of mpv's `track-list` JSON; only the fields used here.
#[derive(Deserialize)]
struct RawTrack {
    id: i64,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    lang: Option<String>,
    #[serde(default)]
    codec: Option<String>,
    #[serde(rename = "demux-channel-count", default)]
    channels: Option<u32>,
    #[serde(default)]
    selected: bool,
    #[serde(default)]
    forced: bool,
    #[serde(rename = "external-filename", default)]
    external_filename: Option<String>,
}

fn channel_label(count: u32) -> String {
    match count {
        1 => "mono".into(),
        2 => "stereo".into(),
        6 => "5.1".into(),
        8 => "7.1".into(),
        n => format!("{n} ch"),
    }
}

fn track_label(t: &RawTrack) -> String {
    let language = t.lang.as_deref().map(language::name);
    let mut details: Vec<String> = Vec::new();
    if let Some(codec) = &t.codec {
        details.push(codec.to_uppercase());
    }
    if let Some(channels) = t.channels {
        details.push(channel_label(channels));
    }
    if t.forced {
        details.push("forced".into());
    }
    let title = t.title.clone().filter(|s| !s.trim().is_empty());
    let head = match (language, title) {
        (Some(lang), Some(title)) if !title.eq_ignore_ascii_case(&lang) => {
            format!("{lang} · {title}")
        }
        (Some(lang), _) => lang,
        (None, Some(title)) => title,
        (None, None) => format!("Track {}", t.id),
    };
    if details.is_empty() {
        head
    } else {
        format!("{head} ({})", details.join(", "))
    }
}

/// Reads the tracks of one type ("audio" or "sub") out of mpv's `track-list` property.
pub fn tracks(track_list_json: &str, kind: &str) -> Vec<Track> {
    let raw: Vec<RawTrack> = serde_json::from_str(track_list_json).unwrap_or_default();
    raw.into_iter()
        .filter(|t| t.kind == kind)
        .map(|t| Track {
            id: t.id,
            label: track_label(&t),
            selected: t.selected,
            external: t.external_filename.is_some(),
            external_url: t.external_filename.clone(),
        })
        .collect()
}

/// The addon subtitles with the one mpv is showing marked as selected.
pub fn addon_subtitle_views(subs: &[AddonSubtitle], loaded: &[Track]) -> Vec<AddonSubtitleView> {
    subs.iter()
        .map(|s| AddonSubtitleView {
            key: s.key.clone(),
            label: s.label.clone(),
            addon_name: s.addon_name.clone(),
            selected: loaded
                .iter()
                .any(|t| t.selected && t.external_url.as_deref() == Some(s.url.as_str())),
        })
        .collect()
}

impl PlaybackState {
    /// Replaces the addon subtitles offered for this video.
    pub fn set_addon_subtitles(&mut self, subs: Vec<AddonSubtitle>) {
        self.addon_subtitle_files = subs;
        self.refresh_addon_subtitles();
    }

    /// Recomputes which addon subtitle is showing, after the track list or the list changed.
    fn refresh_addon_subtitles(&mut self) {
        self.addon_subtitles =
            addon_subtitle_views(&self.addon_subtitle_files, &self.subtitle_tracks);
    }

    /// Applies one observed property. Returns whether the change is worth sending right away
    /// (the position changes every frame and is sent at a slower pace).
    pub fn apply(&mut self, name: &str, value: &PropertyValue) -> bool {
        match (name, value) {
            ("time-pos", PropertyValue::Double(v)) => {
                self.position = *v;
                return false;
            }
            ("duration", PropertyValue::Double(v)) => self.duration = *v,
            ("volume", PropertyValue::Double(v)) => self.volume = *v,
            ("speed", PropertyValue::Double(v)) => self.speed = *v,
            ("pause", PropertyValue::Flag(v)) => self.paused = *v,
            ("mute", PropertyValue::Flag(v)) => self.muted = *v,
            ("paused-for-cache", PropertyValue::Flag(v)) => self.buffering = *v,
            ("seeking", PropertyValue::Flag(v)) => self.seeking = *v,
            ("seekable", PropertyValue::Flag(v)) => self.seekable = *v,
            ("track-list", PropertyValue::Text(json)) => {
                self.audio_tracks = tracks(json, "audio");
                self.subtitle_tracks = tracks(json, "sub");
                self.refresh_addon_subtitles();
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACK_LIST: &str = r#"[
        {"id":1,"type":"video","codec":"hevc","selected":true},
        {"id":1,"type":"audio","lang":"ger","codec":"eac3","demux-channel-count":6,"selected":true},
        {"id":2,"type":"audio","title":"English","lang":"eng","codec":"aac","demux-channel-count":2,"selected":false},
        {"id":3,"type":"audio","title":"Commentary"},
        {"id":4,"type":"audio"},
        {"id":1,"type":"sub","lang":"eng","codec":"subrip"},
        {"id":2,"type":"sub","lang":"ger","forced":true,"codec":"ass"},
        {"id":3,"type":"sub","title":"English","lang":"eng","codec":"subrip","selected":true,
         "external-filename":"https://subs.example/1.srt"}
    ]"#;

    #[test]
    fn reads_audio_tracks() {
        let audio = tracks(TRACK_LIST, "audio");
        let labels: Vec<&str> = audio.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "German (EAC3, 5.1)",
                "English (AAC, stereo)",
                "Commentary",
                "Track 4"
            ]
        );
        assert!(audio[0].selected && !audio[1].selected);
    }

    #[test]
    fn reads_subtitle_tracks() {
        let subs = tracks(TRACK_LIST, "sub");
        let labels: Vec<&str> = subs.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "English (SUBRIP)",
                "German (ASS, forced)",
                "English (SUBRIP)"
            ]
        );
        assert_eq!(
            subs[2].external_url.as_deref(),
            Some("https://subs.example/1.srt")
        );
    }

    #[test]
    fn marks_the_addon_subtitle_mpv_shows() {
        let addon = |key: &str, url: &str| AddonSubtitle {
            key: key.into(),
            url: url.into(),
            lang: "eng".into(),
            lang_code: Some("en".into()),
            label: "English".into(),
            addon_name: "OpenSubtitles".into(),
        };
        let subs = vec![
            addon("a", "https://subs.example/1.srt"),
            addon("b", "https://subs.example/2.srt"),
        ];
        let mut state = PlaybackState::default();
        state.set_addon_subtitles(subs);
        assert!(!state.addon_subtitles[0].selected);
        state.apply("track-list", &PropertyValue::Text(TRACK_LIST.into()));
        assert!(state.addon_subtitles[0].selected);
        assert!(!state.addon_subtitles[1].selected);
        assert_eq!(
            state.subtitle_tracks.iter().filter(|t| !t.external).count(),
            2
        );
    }

    #[test]
    fn bad_track_list_gives_no_tracks() {
        assert!(tracks("not json", "audio").is_empty());
        assert!(tracks("[]", "sub").is_empty());
    }

    #[test]
    fn position_changes_are_not_urgent() {
        let mut state = PlaybackState::default();
        assert!(!state.apply("time-pos", &PropertyValue::Double(12.5)));
        assert_eq!(state.position, 12.5);
        assert!(state.apply("pause", &PropertyValue::Flag(true)));
        assert!(state.paused);
        assert!(state.apply("track-list", &PropertyValue::Text(TRACK_LIST.into())));
        assert_eq!(state.audio_tracks.len(), 4);
        assert!(!state.apply("unknown", &PropertyValue::Double(1.0)));
    }
}
