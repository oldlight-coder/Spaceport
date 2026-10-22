//! The Android player: Media3 (ExoPlayer) in the Kotlin plugin, driven through the same calls
//! the rest of the player module makes to mpv. mpv commands and properties are translated to
//! the plugin's operations, and the plugin's state is polled and turned back into mpv-style
//! events, so the queue, fallback, resume and progress code is shared with Windows.
//!
//! The Kotlin side is `SpaceportPlugin.kt` (commands `playerOpen`, `playerOp`, `playerPoll`,
//! `playerClose`).

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use super::mpv::{Event, PropertyValue};

/// One operation for the Kotlin player.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Op {
    /// Plays `url`, from `start` seconds when given.
    Load {
        url: String,
        start: Option<f64>,
    },
    TogglePause,
    Seek {
        seconds: f64,
        relative: bool,
    },
    ToggleMute,
    /// 0 to 100 (mpv allows up to 130; Android cannot go above full volume).
    Volume {
        value: f64,
    },
    Speed {
        value: f64,
    },
    AudioTrack {
        id: i64,
    },
    /// A subtitle track, or none.
    SubtitleTrack {
        id: Option<i64>,
    },
    /// Adds a subtitle file by URL and shows it.
    AddSubtitle {
        url: String,
        title: String,
        lang: String,
    },
    /// Languages to pick audio or subtitle tracks in, best first.
    AudioLanguages {
        languages: Vec<String>,
    },
    SubtitleLanguages {
        languages: Vec<String>,
    },
    /// One subtitle style setting, by its mpv name (`sub-scale`, `sub-color`, ...).
    Style {
        name: String,
        value: String,
    },
}

/// What an mpv command means for the Kotlin player. `Ok(None)` is a command with nothing to do
/// on Android; `quit` is handled by the caller.
pub fn command_op(args: &[&str]) -> Result<Option<Op>, String> {
    let op = match args {
        ["loadfile", url, rest @ ..] => {
            let start = rest
                .iter()
                .find_map(|a| a.strip_prefix("start="))
                .and_then(|s| s.parse::<f64>().ok());
            Op::Load {
                url: url.to_string(),
                start,
            }
        }
        ["cycle", "pause"] => Op::TogglePause,
        ["cycle", "mute"] => Op::ToggleMute,
        ["seek", seconds, mode] => Op::Seek {
            seconds: seconds.parse().map_err(|_| "bad seek position")?,
            relative: *mode == "relative",
        },
        ["sub-add", url, _flag, rest @ ..] => {
            let title = rest.first().copied().unwrap_or("Subtitles");
            let lang = rest.get(1).copied().unwrap_or("");
            Op::AddSubtitle {
                url: url.to_string(),
                title: title.to_string(),
                lang: lang.to_string(),
            }
        }
        // Subtitle timing cannot be shifted in Media3; the control is hidden on Android.
        ["add", "sub-delay", _] => return Ok(None),
        _ => return Err(format!("not supported on Android: {}", args.join(" "))),
    };
    Ok(Some(op))
}

/// What setting an mpv property means for the Kotlin player. `None` when it has no meaning
/// there (the window title, mpv-only subtitle rules).
pub fn property_op(name: &str, value: &str) -> Option<Op> {
    let number = || value.parse::<f64>().ok();
    let list = || {
        value
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect::<Vec<_>>()
    };
    Some(match name {
        "volume" => Op::Volume {
            value: number()?.clamp(0.0, 100.0),
        },
        "speed" => Op::Speed { value: number()? },
        "aid" => Op::AudioTrack {
            id: value.parse().ok()?,
        },
        "sid" => Op::SubtitleTrack {
            id: value.parse().ok(),
        },
        "alang" => Op::AudioLanguages { languages: list() },
        "slang" => Op::SubtitleLanguages { languages: list() },
        "sub-scale" | "sub-color" | "sub-border-style" | "sub-back-color" | "sub-pos" => {
            Op::Style {
                name: name.to_string(),
                value: value.to_string(),
            }
        }
        _ => return None,
    })
}

/// The Kotlin player's state, as `playerPoll` returns it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Poll {
    /// What happened since the last poll: `loaded`, `ended`, `back`, or `error:<text>`.
    pub events: Vec<String>,
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
    pub buffering: bool,
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub seekable: bool,
    /// Audio and subtitle tracks in the shape of mpv's `track-list`.
    pub tracks: serde_json::Value,
}

/// Turns a poll into mpv-style events: one per thing that happened, then one per property that
/// changed since `last` (all of them on the first poll).
pub fn poll_events(last: Option<&Poll>, now: &Poll) -> VecDeque<Event> {
    let mut events: VecDeque<Event> = now
        .events
        .iter()
        .map(|e| match e.as_str() {
            "loaded" => Event::FileLoaded,
            "ended" => Event::EndOfFile,
            "back" => Event::CloseRequested,
            other => match other.strip_prefix("error:") {
                Some(text) => Event::Failed(text.to_string()),
                None => Event::Other,
            },
        })
        .collect();
    let mut changed = |name: &str, old: Option<PropertyValue>, new: PropertyValue| {
        if old.as_ref() != Some(&new) {
            events.push_back(Event::Property {
                name: name.to_string(),
                value: new,
            });
        }
    };
    let double = |f: fn(&Poll) -> f64| last.map(|p| PropertyValue::Double(f(p)));
    let flag = |f: fn(&Poll) -> bool| last.map(|p| PropertyValue::Flag(f(p)));
    changed(
        "duration",
        double(|p| p.duration),
        PropertyValue::Double(now.duration),
    );
    changed(
        "time-pos",
        double(|p| p.position),
        PropertyValue::Double(now.position),
    );
    changed("pause", flag(|p| p.paused), PropertyValue::Flag(now.paused));
    changed(
        "paused-for-cache",
        flag(|p| p.buffering),
        PropertyValue::Flag(now.buffering),
    );
    changed(
        "volume",
        double(|p| p.volume),
        PropertyValue::Double(now.volume),
    );
    changed("mute", flag(|p| p.muted), PropertyValue::Flag(now.muted));
    changed(
        "speed",
        double(|p| p.speed),
        PropertyValue::Double(now.speed),
    );
    changed(
        "seekable",
        flag(|p| p.seekable),
        PropertyValue::Flag(now.seekable),
    );
    changed(
        "track-list",
        last.map(|p| PropertyValue::Text(p.tracks.to_string())),
        PropertyValue::Text(now.tracks.to_string()),
    );
    events
}

#[cfg(target_os = "android")]
pub use android::{Mpv, MpvLib};

#[cfg(target_os = "android")]
mod android {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{channel, Sender};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use tauri::plugin::PluginHandle;
    use tauri::Wry;

    use super::{command_op, poll_events, property_op, Op, Poll};
    use crate::player::mpv::Event;

    /// How often the Kotlin player's state is read.
    const POLL_INTERVAL: Duration = Duration::from_millis(250);

    /// Stands in for the loaded libmpv: the handle to the Kotlin plugin.
    pub struct MpvLib(pub PluginHandle<Wry>);

    /// Messages for the worker thread that makes the plugin calls.
    enum Message {
        Open,
        Op(Op),
        Close,
    }

    /// One playback session on the Kotlin player.
    pub struct Mpv {
        plugin: PluginHandle<Wry>,
        /// Plugin calls wait for Android's UI thread, which may be the caller's own thread (the
        /// main thread is the UI thread on Android), so they are made on a worker in order.
        worker: Mutex<Sender<Message>>,
        quitting: AtomicBool,
        pending: Mutex<VecDeque<Event>>,
        last: Mutex<Option<Poll>>,
    }

    impl Mpv {
        /// Opens the player view. Of the start-up options only the audio languages apply.
        pub fn new(lib: Arc<MpvLib>, options: &[(&str, &str)]) -> Result<Self, String> {
            let (sender, receiver) = channel::<Message>();
            let plugin = lib.0.clone();
            let worker_plugin = plugin.clone();
            std::thread::spawn(move || {
                for message in receiver {
                    let (command, payload) = match message {
                        Message::Open => ("playerOpen", serde_json::json!({})),
                        Message::Op(op) => {
                            ("playerOp", serde_json::to_value(&op).unwrap_or_default())
                        }
                        Message::Close => ("playerClose", serde_json::json!({})),
                    };
                    // A failed operation shows in the polled state (or not at all, for a style).
                    let _ = worker_plugin.run_mobile_plugin::<serde_json::Value>(command, payload);
                }
            });
            let mpv = Self {
                plugin,
                worker: Mutex::new(sender),
                quitting: AtomicBool::new(false),
                pending: Mutex::new(VecDeque::new()),
                last: Mutex::new(None),
            };
            mpv.send(Message::Open);
            for (name, value) in options {
                if let Some(op) = property_op(name, value) {
                    mpv.send(Message::Op(op));
                }
            }
            Ok(mpv)
        }

        fn send(&self, message: Message) {
            let _ = self.worker.lock().unwrap().send(message);
        }

        pub fn command(&self, args: &[&str]) -> Result<(), String> {
            if args == ["quit"] {
                self.quitting.store(true, Ordering::SeqCst);
                return Ok(());
            }
            if let Some(op) = command_op(args)? {
                self.send(Message::Op(op));
            }
            Ok(())
        }

        pub fn set_property(&self, name: &str, value: &str) -> Result<(), String> {
            if let Some(op) = property_op(name, value) {
                self.send(Message::Op(op));
            }
            Ok(())
        }

        /// Every property the player module reads is always polled.
        pub fn observe(&self, _id: u64, _name: &str, _format: i32) -> Result<(), String> {
            Ok(())
        }

        /// Waits up to `timeout` seconds for the next event, reading the player's state at
        /// `POLL_INTERVAL`. Runs on the event thread, never the UI thread.
        pub fn wait_event(&self, timeout: f64) -> Event {
            if self.quitting.load(Ordering::SeqCst) {
                return Event::Shutdown;
            }
            if let Some(event) = self.pending.lock().unwrap().pop_front() {
                return event;
            }
            let wait = Duration::from_secs_f64(timeout.max(0.0)).min(POLL_INTERVAL);
            std::thread::sleep(wait);
            if self.quitting.load(Ordering::SeqCst) {
                return Event::Shutdown;
            }
            let Ok(now) = self
                .plugin
                .run_mobile_plugin::<Poll>("playerPoll", serde_json::json!({}))
            else {
                return Event::None;
            };
            let mut last = self.last.lock().unwrap();
            let mut events = poll_events(last.as_ref(), &now);
            *last = Some(now);
            drop(last);
            let first = events.pop_front().unwrap_or(Event::None);
            self.pending.lock().unwrap().extend(events);
            first
        }
    }

    impl Drop for Mpv {
        fn drop(&mut self) {
            // The worker makes the call and then ends, as its channel closes.
            self.send(Message::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_load_with_and_without_start() {
        assert_eq!(
            command_op(&["loadfile", "http://127.0.0.1:1/a", "replace"]).unwrap(),
            Some(Op::Load {
                url: "http://127.0.0.1:1/a".into(),
                start: None
            })
        );
        assert_eq!(
            command_op(&["loadfile", "https://x/b", "replace", "-1", "start=754.0"]).unwrap(),
            Some(Op::Load {
                url: "https://x/b".into(),
                start: Some(754.0)
            })
        );
    }

    #[test]
    fn translates_controls() {
        assert_eq!(
            command_op(&["cycle", "pause"]).unwrap(),
            Some(Op::TogglePause)
        );
        assert_eq!(
            command_op(&["seek", "-10.00", "relative"]).unwrap(),
            Some(Op::Seek {
                seconds: -10.0,
                relative: true
            })
        );
        assert_eq!(
            command_op(&[
                "sub-add",
                "https://s/1.srt",
                "select",
                "English · OS",
                "eng"
            ])
            .unwrap(),
            Some(Op::AddSubtitle {
                url: "https://s/1.srt".into(),
                title: "English · OS".into(),
                lang: "eng".into()
            })
        );
        assert_eq!(command_op(&["add", "sub-delay", "0.100"]).unwrap(), None);
        assert!(command_op(&["frame-step"]).is_err());
        assert!(command_op(&["seek", "soon", "absolute"]).is_err());
    }

    #[test]
    fn translates_properties() {
        assert_eq!(
            property_op("volume", "130"),
            Some(Op::Volume { value: 100.0 })
        );
        assert_eq!(
            property_op("sid", "no"),
            Some(Op::SubtitleTrack { id: None })
        );
        assert_eq!(
            property_op("sid", "3"),
            Some(Op::SubtitleTrack { id: Some(3) })
        );
        assert_eq!(
            property_op("alang", "de,ger,deu"),
            Some(Op::AudioLanguages {
                languages: vec!["de".into(), "ger".into(), "deu".into()]
            })
        );
        assert!(matches!(
            property_op("sub-pos", "95"),
            Some(Op::Style { .. })
        ));
        assert_eq!(property_op("force-media-title", "Film"), None);
        assert_eq!(property_op("subs-with-matching-audio", "forced"), None);
    }

    #[test]
    fn first_poll_reports_everything_then_only_changes() {
        let first = Poll {
            events: vec!["loaded".into()],
            position: 1.0,
            duration: 1400.0,
            volume: 100.0,
            speed: 1.0,
            seekable: true,
            tracks: serde_json::json!([]),
            ..Poll::default()
        };
        let events = poll_events(None, &first);
        assert_eq!(events[0], Event::FileLoaded);
        assert_eq!(events.len(), 10);

        let second = Poll {
            events: vec!["error:Source error".into()],
            position: 1.25,
            ..first.clone()
        };
        let events: Vec<Event> = poll_events(Some(&first), &second).into();
        assert_eq!(
            events,
            [
                Event::Failed("Source error".into()),
                Event::Property {
                    name: "time-pos".into(),
                    value: PropertyValue::Double(1.25)
                }
            ]
        );
        assert!(poll_events(
            Some(&second),
            &Poll {
                events: vec![],
                ..second.clone()
            }
        )
        .is_empty());
    }

    #[test]
    fn back_asks_to_close() {
        let poll = Poll {
            events: vec!["back".into(), "ended".into()],
            ..Poll::default()
        };
        let events = poll_events(Some(&poll), &poll);
        assert_eq!(Vec::from(events), [Event::CloseRequested, Event::EndOfFile]);
    }
}
