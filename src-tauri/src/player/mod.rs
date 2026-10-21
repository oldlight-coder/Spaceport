//! Video playback with libmpv, drawn into a child window of the main window, with Spaceport's
//! controls in a transparent window kept over it.

mod mpv;
mod queue;
mod state;
mod surface;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::stremio::subtitles::AddonSubtitle;
use mpv::{Event, Mpv, MpvLib, FORMAT_DOUBLE, FORMAT_FLAG, FORMAT_STRING};
pub use queue::Candidate;
use queue::{Action, PlayQueue};
pub use state::PlaybackState as PlaybackStateView;
use state::PlaybackState;
use surface::Surface;

/// The DLL `pnpm run setup` places in `src-tauri/lib` and the installer puts next to the app.
const LIBRARY_FILE: &str = "libmpv-2.dll";

/// Label of the transparent window holding the player controls.
pub const CONTROLS_WINDOW: &str = "player-controls";

/// Options applied before mpv starts. Everything that could load code or config from outside
/// the app is off: no user config files, no user scripts, no youtube-dl helper. mpv's own
/// controls and key bindings are off too; the controls window handles input.
const BASE_OPTIONS: &[(&str, &str)] = &[
    ("config", "no"),
    ("load-scripts", "no"),
    ("ytdl", "no"),
    ("terminal", "no"),
    ("hwdec", "auto-safe"),
    ("force-window", "yes"),
    ("idle", "yes"),
    ("keep-open", "no"),
    ("osc", "no"),
    ("input-default-bindings", "no"),
    ("input-vo-keyboard", "no"),
    ("input-cursor", "no"),
    ("network-timeout", "20"),
];

/// Properties sent to the controls window, with the format each is read in.
const OBSERVED: &[(&str, i32)] = &[
    ("time-pos", FORMAT_DOUBLE),
    ("duration", FORMAT_DOUBLE),
    ("volume", FORMAT_DOUBLE),
    ("speed", FORMAT_DOUBLE),
    ("pause", FORMAT_FLAG),
    ("mute", FORMAT_FLAG),
    ("paused-for-cache", FORMAT_FLAG),
    ("seeking", FORMAT_FLAG),
    ("seekable", FORMAT_FLAG),
    ("track-list", FORMAT_STRING),
];

/// The position changes every frame; the controls get it at most this often.
const POSITION_INTERVAL: Duration = Duration::from_millis(250);

/// What the UI hears about playback, as the `player` event.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlayerEvent {
    /// A stream started; `label` names which one.
    Loaded {
        label: String,
    },
    /// The previous stream failed and the next ranked one is being tried.
    Fallback {
        label: String,
        reason: String,
    },
    Ended,
    /// Every candidate failed.
    Failed {
        message: String,
    },
    Closed,
}

/// A request from the controls window.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum Control {
    TogglePause,
    /// Seek to `seconds`, or by `seconds` when `relative`.
    Seek {
        seconds: f64,
        relative: bool,
    },
    SetVolume {
        volume: f64,
    },
    ToggleMute,
    SetSpeed {
        speed: f64,
    },
    SetAudioTrack {
        id: i64,
    },
    /// Subtitles off, an embedded track (`track`), or an addon subtitle by `key`.
    SetSubtitle {
        track: Option<i64>,
        key: Option<String>,
    },
    /// Switch to another of the ranked streams, keeping the position.
    SetSource {
        index: usize,
    },
    ToggleFullscreen,
    /// Leave fullscreen, or close the player when not in fullscreen.
    Back,
    Close,
}

/// The candidates of the current title, shared with the event thread.
type SharedQueue = Arc<Mutex<Option<PlayQueue>>>;
type SharedState = Arc<Mutex<PlaybackState>>;

struct Session {
    mpv: Arc<Mpv>,
    surface: Surface,
    queue: SharedQueue,
    state: SharedState,
}

#[derive(Default)]
pub struct Player {
    lib: Mutex<Option<Arc<MpvLib>>>,
    session: Mutex<Option<Session>>,
}

/// Where libmpv may be, in order: next to the installed app (bundle resources), next to the
/// executable, and in debug builds the `src-tauri/lib` folder `pnpm run setup` fills, since
/// `tauri dev` does not always copy resources beside the dev executable.
fn library_candidates(app: &AppHandle) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        candidates.push(dir.join(LIBRARY_FILE));
        candidates.push(dir.join("lib").join(LIBRARY_FILE));
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
    {
        candidates.push(dir.join(LIBRARY_FILE));
    }
    if cfg!(debug_assertions) {
        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("lib")
                .join(LIBRARY_FILE),
        );
    }
    candidates.dedup();
    candidates
}

impl Player {
    /// Loads libmpv once it is found. A failure is not remembered, so the next Play tries again.
    fn lib(&self, app: &AppHandle) -> Result<Arc<MpvLib>, String> {
        let mut lib = self.lib.lock().unwrap();
        if let Some(loaded) = lib.as_ref() {
            return Ok(loaded.clone());
        }
        let candidates = library_candidates(app);
        let Some(path) = candidates.iter().find(|p| p.is_file()) else {
            let tried: Vec<String> = candidates.iter().map(|p| p.display().to_string()).collect();
            return Err(format!(
                "the player library {LIBRARY_FILE} was not found (run `pnpm run setup`). Looked in: {}",
                tried.join("; ")
            ));
        };
        let loaded = MpvLib::load(path)?;
        *lib = Some(loaded.clone());
        Ok(loaded)
    }

    /// Plays the first of `candidates` (ranked best first) and falls back down the list when a
    /// stream fails. Reuses the open player if there is one. Main thread only.
    /// `audio_languages` is mpv's `alang` list for picking the dub.
    pub fn open(
        &self,
        app: &AppHandle,
        candidates: Vec<Candidate>,
        title: &str,
        audio_languages: &str,
    ) -> Result<(), String> {
        let (queue, first) =
            PlayQueue::new(candidates, Instant::now()).ok_or("there is no stream to play")?;
        let mut session = self.session.lock().unwrap();
        if let Some(existing) = session.as_ref() {
            {
                let mut state = existing.state.lock().unwrap();
                state.title = title.to_string();
                state.sources = queue.labels();
                state.source_index = 0;
                state.set_addon_subtitles(Vec::new());
            }
            existing.mpv.set_property("force-media-title", title)?;
            *existing.queue.lock().unwrap() = Some(queue);
            return apply(&existing.mpv, first);
        }

        let lib = self.lib(app)?;
        let parent = window_handle(&main_window(app)?)?;
        let surface = Surface::create(parent)?;
        let wid = surface.id().to_string();
        let mut options = BASE_OPTIONS.to_vec();
        options.push(("wid", &wid));
        if !audio_languages.is_empty() {
            options.push(("alang", audio_languages));
        }
        let mpv = match Mpv::new(lib, &options) {
            Ok(mpv) => Arc::new(mpv),
            Err(e) => {
                surface.destroy();
                return Err(e);
            }
        };
        for (id, (name, format)) in OBSERVED.iter().enumerate() {
            mpv.observe(id as u64 + 1, name, *format)?;
        }
        mpv.set_property("force-media-title", title)?;

        let queue: SharedQueue = Arc::new(Mutex::new(Some(queue)));
        let state: SharedState = Arc::new(Mutex::new(PlaybackState {
            title: title.to_string(),
            seekable: true,
            sources: queue
                .lock()
                .unwrap()
                .as_ref()
                .map(PlayQueue::labels)
                .unwrap_or_default(),
            ..PlaybackState::default()
        }));
        spawn_event_thread(app.clone(), mpv.clone(), queue.clone(), state.clone());
        apply(&mpv, first)?;
        *session = Some(Session {
            mpv,
            surface,
            queue,
            state,
        });
        Ok(())
    }

    /// Carries out a request from the controls window.
    pub fn control(&self, app: &AppHandle, control: Control) -> Result<(), String> {
        let session = self.session.lock().unwrap();
        let Some(session) = session.as_ref() else {
            return Ok(());
        };
        let mpv = &session.mpv;
        match control {
            Control::TogglePause => mpv.command(&["cycle", "pause"]),
            Control::Seek { seconds, relative } => {
                let mode = if relative { "relative" } else { "absolute" };
                mpv.command(&["seek", &format!("{seconds:.2}"), mode])
            }
            Control::SetVolume { volume } => {
                mpv.set_property("volume", &format!("{:.0}", volume.clamp(0.0, 130.0)))
            }
            Control::ToggleMute => mpv.command(&["cycle", "mute"]),
            Control::SetSpeed { speed } => {
                mpv.set_property("speed", &format!("{:.2}", speed.clamp(0.25, 4.0)))
            }
            Control::SetAudioTrack { id } => mpv.set_property("aid", &id.to_string()),
            Control::SetSubtitle { track, key } => {
                if let Some(id) = track {
                    return mpv.set_property("sid", &id.to_string());
                }
                let Some(key) = key else {
                    return mpv.set_property("sid", "no");
                };
                let state = session.state.lock().unwrap();
                let sub = state
                    .addon_subtitle_files
                    .iter()
                    .find(|s| s.key == key)
                    .ok_or("that subtitle is no longer offered")?;
                // Loaded before: switch back to it instead of downloading it again.
                let loaded = state
                    .subtitle_tracks
                    .iter()
                    .find(|t| t.external_url.as_deref() == Some(sub.url.as_str()));
                match loaded {
                    Some(track) => mpv.set_property("sid", &track.id.to_string()),
                    None => {
                        let title = format!("{} · {}", sub.label, sub.addon_name);
                        mpv.command(&["sub-add", &sub.url, "select", &title, &sub.lang])
                    }
                }
            }
            Control::SetSource { index } => {
                let action = session
                    .queue
                    .lock()
                    .unwrap()
                    .as_mut()
                    .and_then(|q| q.select(index, Instant::now()))
                    .ok_or("that source is not available")?;
                {
                    let mut state = session.state.lock().unwrap();
                    state.source_index = index;
                    state.stream.clear();
                    let _ = app.emit("player-state", state.clone());
                }
                apply(mpv, action)
            }
            Control::ToggleFullscreen => {
                let window = main_window(app)?;
                let full = window.is_fullscreen().map_err(|e| e.to_string())?;
                window.set_fullscreen(!full).map_err(|e| e.to_string())
            }
            Control::Back => {
                let window = main_window(app)?;
                if window.is_fullscreen().unwrap_or(false) {
                    window.set_fullscreen(false).map_err(|e| e.to_string())
                } else {
                    mpv.command(&["quit"])
                }
            }
            Control::Close => mpv.command(&["quit"]),
        }
    }

    /// Offers the subtitles the addons returned for the playing video.
    pub fn set_addon_subtitles(&self, app: &AppHandle, subs: Vec<AddonSubtitle>) {
        let session = self.session.lock().unwrap();
        if let Some(session) = session.as_ref() {
            let mut state = session.state.lock().unwrap();
            state.set_addon_subtitles(subs);
            let _ = app.emit("player-state", state.clone());
        }
    }

    /// The current state, for a controls window that has just opened.
    pub fn state(&self) -> Option<PlaybackState> {
        let session = self.session.lock().unwrap();
        session.as_ref().map(|s| s.state.lock().unwrap().clone())
    }

    /// Asks mpv to quit; the event thread then tears the session down.
    pub fn close(&self) {
        if let Some(session) = self.session.lock().unwrap().as_ref() {
            let _ = session.mpv.command(&["quit"]);
        }
    }

    /// Keeps the video window the size of the main window. Main thread only.
    pub fn resize(&self, width: u32, height: u32) {
        if let Some(session) = self.session.lock().unwrap().as_ref() {
            session.surface.resize(width, height);
        }
    }

    pub fn is_open(&self) -> bool {
        self.session.lock().unwrap().is_some()
    }

    fn take_session(&self) -> Option<Session> {
        self.session.lock().unwrap().take()
    }
}

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("main")
        .ok_or_else(|| "the main window is gone".to_string())
}

/// Opens the transparent controls window over the main window's client area. Must not run on
/// the main thread or in a synchronous command: creating a web view there deadlocks on Windows.
pub fn open_controls(app: &AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window(CONTROLS_WINDOW) {
        let _ = existing.set_focus();
        return Ok(());
    }
    let main = main_window(app)?;
    let window =
        WebviewWindowBuilder::new(app, CONTROLS_WINDOW, WebviewUrl::App("player.html".into()))
            .title("Spaceport player")
            .parent(&main)
            .map_err(|e| e.to_string())?
            .transparent(true)
            .decorations(false)
            .shadow(false)
            .resizable(false)
            .skip_taskbar(true)
            .focused(true)
            .visible(false)
            .build()
            .map_err(|e| e.to_string())?;
    place_controls(app);
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    Ok(())
}

/// Puts the controls window exactly over the main window's client area (physical pixels).
pub fn place_controls(app: &AppHandle) {
    let (Some(main), Some(controls)) = (
        app.get_webview_window("main"),
        app.get_webview_window(CONTROLS_WINDOW),
    ) else {
        return;
    };
    if let (Ok(position), Ok(size)) = (main.inner_position(), main.inner_size()) {
        let _ = controls.set_position(tauri::Position::Physical(position));
        let _ = controls.set_size(tauri::Size::Physical(size));
    }
}

#[cfg(windows)]
fn window_handle(window: &WebviewWindow) -> Result<isize, String> {
    window
        .hwnd()
        .map(|hwnd| hwnd.0 as isize)
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
fn window_handle(_window: &WebviewWindow) -> Result<isize, String> {
    Err("the player is only available on Windows".to_string())
}

/// Carries out a queue decision: loads a stream (optionally from a position).
fn apply(mpv: &Mpv, action: Action) -> Result<(), String> {
    match action {
        Action::Load { url, start, .. } => match start {
            Some(seconds) => {
                let start = format!("start={seconds:.1}");
                mpv.command(&["loadfile", &url, "replace", "-1", &start])
            }
            None => mpv.command(&["loadfile", &url, "replace"]),
        },
        Action::GiveUp { .. } | Action::Wait => Ok(()),
    }
}

/// Acts on a queue decision and tells the UI about it.
fn follow(app: &AppHandle, mpv: &Mpv, action: Action, reason: &str) {
    match &action {
        Action::Load { label, .. } => {
            let _ = app.emit(
                "player",
                PlayerEvent::Fallback {
                    label: label.clone(),
                    reason: reason.to_string(),
                },
            );
            if let Err(e) = apply(mpv, action) {
                let _ = app.emit("player", PlayerEvent::Failed { message: e });
                let _ = mpv.command(&["quit"]);
            }
        }
        Action::GiveUp { tried, last_error } => {
            let message = if *tried == 1 {
                format!("The stream would not play ({last_error}).")
            } else {
                format!("None of the {tried} streams would play (last: {last_error}).")
            };
            let _ = app.emit("player", PlayerEvent::Failed { message });
            let _ = mpv.command(&["quit"]);
        }
        Action::Wait => {}
    }
}

/// Forwards mpv events to the UI, switches streams on failure, and removes the video and
/// controls windows once mpv shuts down.
fn spawn_event_thread(app: AppHandle, mpv: Arc<Mpv>, queue: SharedQueue, state: SharedState) {
    std::thread::spawn(move || {
        let mut last_sent = Instant::now();
        loop {
            // Wake at least once a second so a stream that never starts can be timed out.
            let event = mpv.wait_event(1.0);
            let now = Instant::now();

            if let Event::Property { name, value } = &event {
                let mut current = state.lock().unwrap();
                let urgent = current.apply(name, value);
                if name == "time-pos" {
                    if let mpv::PropertyValue::Double(v) = value {
                        if let Some(q) = queue.lock().unwrap().as_mut() {
                            q.on_position(*v);
                        }
                    }
                }
                if name == "duration" {
                    if let mpv::PropertyValue::Double(v) = value {
                        if let Some(q) = queue.lock().unwrap().as_mut() {
                            q.on_duration(*v);
                        }
                    }
                }
                if urgent || now.duration_since(last_sent) >= POSITION_INTERVAL {
                    last_sent = now;
                    let _ = app.emit("player-state", current.clone());
                }
                continue;
            }

            let mut guard = queue.lock().unwrap();
            let Some(q) = guard.as_mut() else {
                if event == Event::Shutdown {
                    break;
                }
                continue;
            };
            match event {
                Event::Shutdown => break,
                Event::FileLoaded => {
                    q.on_loaded();
                    let label = q.current_label().to_string();
                    let index = q.index();
                    drop(guard);
                    let mut current = state.lock().unwrap();
                    current.stream = label.clone();
                    current.source_index = index;
                    let _ = app.emit("player-state", current.clone());
                    let _ = app.emit("player", PlayerEvent::Loaded { label });
                }
                Event::Failed(error) => {
                    let action = q.on_failed(&error, now);
                    drop(guard);
                    follow(&app, &mpv, action, &format!("Stream failed ({error})"));
                }
                Event::EndOfFile => {
                    if q.finished_normally() {
                        drop(guard);
                        let _ = app.emit("player", PlayerEvent::Ended);
                    } else {
                        let action = q.on_end_of_file(now);
                        drop(guard);
                        follow(&app, &mpv, action, "Stream stopped early");
                    }
                }
                Event::None => {
                    let action = q.on_tick(now);
                    drop(guard);
                    follow(&app, &mpv, action, "Stream did not start");
                }
                _ => {}
            }
        }
        drop(mpv);
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let player = handle.state::<crate::AppState>();
            if let Some(session) = player.player.take_session() {
                // Destroying the last Arc ends mpv; the window goes after it stops drawing.
                drop(session.mpv);
                session.surface.destroy();
            }
            if let Some(controls) = handle.get_webview_window(CONTROLS_WINDOW) {
                let _ = controls.destroy();
            }
            if let Some(main) = handle.get_webview_window("main") {
                let _ = main.set_fullscreen(false);
                let _ = main.set_focus();
            }
            let _ = handle.emit("player", PlayerEvent::Closed);
        });
    });
}
