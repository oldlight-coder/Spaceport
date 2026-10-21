//! The playback state the controls window shows, built from mpv property changes.

use serde::{Deserialize, Serialize};

use super::mpv::PropertyValue;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTrack {
    pub id: i64,
    /// A readable name: the track title, else language and codec.
    pub label: String,
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
    pub audio_tracks: Vec<AudioTrack>,
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

/// Reads the audio tracks out of mpv's `track-list` property.
pub fn audio_tracks(track_list_json: &str) -> Vec<AudioTrack> {
    let tracks: Vec<RawTrack> = serde_json::from_str(track_list_json).unwrap_or_default();
    tracks
        .into_iter()
        .filter(|t| t.kind == "audio")
        .map(|t| {
            let details: Vec<String> = [
                t.lang.clone().map(|l| l.to_uppercase()),
                t.codec.clone().map(|c| c.to_uppercase()),
                t.channels.map(channel_label),
            ]
            .into_iter()
            .flatten()
            .collect();
            let label = match (t.title.filter(|s| !s.trim().is_empty()), details.is_empty()) {
                (Some(title), true) => title,
                (Some(title), false) => format!("{title} ({})", details.join(", ")),
                (None, false) => details.join(", "),
                (None, true) => format!("Track {}", t.id),
            };
            AudioTrack {
                id: t.id,
                label,
                selected: t.selected,
            }
        })
        .collect()
}

impl PlaybackState {
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
            ("track-list", PropertyValue::Text(json)) => self.audio_tracks = audio_tracks(json),
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
        {"id":1,"type":"sub","lang":"eng"}
    ]"#;

    #[test]
    fn reads_audio_tracks() {
        let tracks = audio_tracks(TRACK_LIST);
        let labels: Vec<&str> = tracks.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "GER, EAC3, 5.1",
                "English (ENG, AAC, stereo)",
                "Commentary",
                "Track 4"
            ]
        );
        assert!(tracks[0].selected && !tracks[1].selected);
    }

    #[test]
    fn bad_track_list_gives_no_tracks() {
        assert!(audio_tracks("not json").is_empty());
        assert!(audio_tracks("[]").is_empty());
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
