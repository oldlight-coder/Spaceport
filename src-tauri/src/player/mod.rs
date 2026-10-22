//! Video playback with libmpv, drawn into a child window of the main window, with Spaceport's
//! controls in a transparent window kept over it. On Android, Media3 plays instead (see `exo`)
//! and the controls are drawn by the main page.

#[cfg(any(target_os = "android", test))]
mod exo;
pub mod layout;
#[cfg_attr(target_os = "android", allow(dead_code))]
mod mpv;
mod queue;
mod state;
mod surface;
mod watch;

#[cfg(not(target_os = "android"))]
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
#[cfg(desktop)]
use tauri::{WebviewUrl, WebviewWindowBuilder};

use crate::aniskip::SkipSegment;
use crate::settings::SubtitleStyle;
use crate::stremio::subtitles::AddonSubtitle;
#[cfg(target_os = "android")]
use exo::{Mpv, MpvLib};
use mpv::{Event, FORMAT_DOUBLE, FORMAT_FLAG, FORMAT_STRING};
#[cfg(not(target_os = "android"))]
use mpv::{Mpv, MpvLib};
pub use queue::Candidate;
use queue::{Action, PlayQueue};
pub use state::PlaybackState as PlaybackStateView;
use state::PlaybackState;
use surface::Surface;
use watch::WatchTracker;
pub use watch::{WatchReport, WatchTarget};

/// The DLL `pnpm run setup` places in `src-tauri/lib` and the installer puts next to the app.
#[cfg(not(target_os = "android"))]
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
    // Torrents may need this long to fetch their file list before the first byte arrives.
    ("network-timeout", "30"),
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
    ("sub-delay", FORMAT_DOUBLE),
    ("sub-scale", FORMAT_DOUBLE),
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
    /// The player is showing; on Android the main page draws its controls from now on.
    Opened,
    /// The video moved between full size and the mini player.
    Layout {
        mini: bool,
    },
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
    /// Move subtitles later (positive) or earlier (negative) by `seconds`.
    AdjustSubtitleDelay {
        seconds: f64,
    },
    ResetSubtitleDelay,
    /// Subtitle size relative to the default, 0.5 to 2.0.
    SetSubtitleScale {
        scale: f64,
    },
    /// Switch to another of the ranked streams, keeping the position.
    SetSource {
        index: usize,
    },
    ToggleFullscreen,
    /// Shrink the video to the mini player in a corner, or bring it back to full size.
    SetMini {
        on: bool,
    },
    /// Leave fullscreen, or close the player when not in fullscreen.
    Back,
    Close,
}

/// The candidates of the current title, shared with the event thread.
type SharedQueue = Arc<Mutex<Option<PlayQueue>>>;
type SharedState = Arc<Mutex<PlaybackState>>;
type SharedWatch = Arc<Mutex<Option<WatchTracker>>>;

/// Receives progress reports (to save locally and in the Stremio library).
pub type Reporter = Arc<dyn Fn(WatchReport) + Send + Sync>;

struct Session {
    /// Tells this session apart from a newer one that replaced it while it was closing.
    id: u64,
    /// Set once mpv was asked to quit; the event thread then tears the session down.
    closing: Arc<AtomicBool>,
    mpv: Arc<Mpv>,
    surface: Surface,
    queue: SharedQueue,
    state: SharedState,
    watch: SharedWatch,
}

/// What to play: the window title, the video, where to start and what comes next.
pub struct PlayRequest {
    pub title: String,
    pub target: WatchTarget,
    /// Seconds to resume from.
    pub start: Option<f64>,
    /// The next episode's label, for the countdown near the end.
    pub next_up: Option<String>,
    /// Skip anime intros, outros and recaps without asking.
    pub auto_skip: bool,
}

/// The user's preferences the player starts with.
pub struct PlayerPrefs {
    /// mpv's `alang` list for picking the dub.
    pub audio_languages: String,
    /// mpv's `slang` list for picking a subtitle track in the file.
    pub subtitle_languages: String,
    pub subtitle_style: SubtitleStyle,
}

/// Applies the subtitle language and style. These are properties rather than start-up options,
/// so an mpv build that lacks one of them still plays.
fn apply_subtitle_prefs(mpv: &Mpv, prefs: &PlayerPrefs) {
    if !prefs.subtitle_languages.is_empty() {
        let _ = mpv.set_property("slang", &prefs.subtitle_languages);
        // Subtitles in a preferred language are turned on only when the audio is in another
        // language; with matching audio only forced (signs and songs) subtitles are shown.
        let _ = mpv.set_property("subs-with-matching-audio", "forced");
    }
    apply_style(mpv, &prefs.subtitle_style);
}

fn apply_style(mpv: &Mpv, style: &SubtitleStyle) {
    for (name, value) in style.mpv_properties() {
        let _ = mpv.set_property(name, &value);
    }
}

/// Numbers sessions, so a closing one can only tear down itself.
static SESSION_IDS: AtomicU64 = AtomicU64::new(1);

/// Asks mpv to quit and marks the session as closing.
fn quit(mpv: &Mpv, closing: &AtomicBool) -> Result<(), String> {
    closing.store(true, Ordering::SeqCst);
    mpv.command(&["quit"])
}

#[derive(Default)]
pub struct Player {
    lib: Mutex<Option<Arc<MpvLib>>>,
    session: Mutex<Option<Session>>,
    reporter: Mutex<Option<Reporter>>,
    /// The video is in the mini player rather than filling the window.
    mini: AtomicBool,
}

/// Sends the tracker's report, if playback started.
fn report(watch: &SharedWatch, reporter: &Option<Reporter>, finished: bool) {
    let report = watch
        .lock()
        .unwrap()
        .as_mut()
        .and_then(|w| w.report(Instant::now(), finished));
    if let (Some(report), Some(reporter)) = (report, reporter) {
        reporter(report);
    }
}

/// Where libmpv may be, in order: next to the installed app (bundle resources), next to the
/// executable, and in debug builds the `src-tauri/lib` folder `pnpm run setup` fills, since
/// `tauri dev` does not always copy resources beside the dev executable.
#[cfg(not(target_os = "android"))]
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
    /// The Kotlin player plugin, on Android.
    #[cfg(target_os = "android")]
    fn lib(&self, app: &AppHandle) -> Result<Arc<MpvLib>, String> {
        let mut lib = self.lib.lock().unwrap();
        if let Some(loaded) = lib.as_ref() {
            return Ok(loaded.clone());
        }
        let native = app
            .try_state::<crate::mobile::Native<tauri::Wry>>()
            .ok_or("the player plugin is not loaded")?;
        let loaded = Arc::new(MpvLib(native.0.clone()));
        *lib = Some(loaded.clone());
        Ok(loaded)
    }

    /// Loads libmpv once it is found. A failure is not remembered, so the next Play tries again.
    #[cfg(not(target_os = "android"))]
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
    pub fn open(
        &self,
        app: &AppHandle,
        candidates: Vec<Candidate>,
        prefs: &PlayerPrefs,
        request: PlayRequest,
    ) -> Result<(), String> {
        let PlayRequest {
            title,
            target,
            start,
            next_up,
            auto_skip,
        } = request;
        // A video started from the page while the mini player runs is watched at full size.
        if self.is_mini() {
            self.set_mini(app, false)?;
        }
        let title = title.as_str();
        let now = Instant::now();
        let (queue, first) =
            PlayQueue::new(candidates, now, start).ok_or("there is no stream to play")?;
        let reporter = self.reporter.lock().unwrap().clone();
        let mut session = self.session.lock().unwrap();
        if session
            .as_ref()
            .is_some_and(|s| s.closing.load(Ordering::SeqCst))
        {
            return Err("the player is still closing; try again".to_string());
        }
        if let Some(existing) = session.as_ref() {
            // Save where the previous video got to before switching.
            report(&existing.watch, &reporter, false);
            *existing.watch.lock().unwrap() = Some(WatchTracker::new(target, now));
            {
                let mut state = existing.state.lock().unwrap();
                state.title = title.to_string();
                state.sources = queue.labels();
                state.source_index = 0;
                state.resumed_from = start;
                state.next_up = next_up;
                state.auto_skip = auto_skip;
                state.skips.clear();
                state.skip_note = None;
                state.set_addon_subtitles(Vec::new());
                // The old video's position and length until the new one reports its own.
                state.position = 0.0;
                state.duration = 0.0;
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
        if !prefs.audio_languages.is_empty() {
            options.push(("alang", &prefs.audio_languages));
        }
        let mpv = match Mpv::new(lib, &options) {
            Ok(mpv) => Arc::new(mpv),
            Err(e) => {
                surface.destroy();
                return Err(e);
            }
        };
        let prepared = OBSERVED
            .iter()
            .enumerate()
            .try_for_each(|(id, (name, format))| mpv.observe(id as u64 + 1, name, *format))
            .and_then(|()| mpv.set_property("force-media-title", title))
            .and_then(|()| {
                apply_subtitle_prefs(&mpv, prefs);
                // Started before the event thread exists, so a failure leaves nothing running.
                apply(&mpv, first)
            });
        if let Err(e) = prepared {
            drop(mpv);
            surface.destroy();
            return Err(e);
        }

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
            resumed_from: start,
            next_up,
            auto_skip,
            ..PlaybackState::default()
        }));
        let watch: SharedWatch = Arc::new(Mutex::new(Some(WatchTracker::new(target, now))));
        let id = SESSION_IDS.fetch_add(1, Ordering::SeqCst);
        let closing = Arc::new(AtomicBool::new(false));
        spawn_event_thread(
            app.clone(),
            id,
            closing.clone(),
            mpv.clone(),
            queue.clone(),
            state.clone(),
            watch.clone(),
            reporter,
        );
        *session = Some(Session {
            id,
            closing,
            mpv,
            surface,
            queue,
            state,
            watch,
        });
        Ok(())
    }

    /// Carries out a request from the controls window.
    pub fn control(&self, app: &AppHandle, control: Control) -> Result<(), String> {
        // Window calls run without the session lock: resizing the window calls back into
        // the player (resize, is_open) on this same thread.
        match control {
            Control::SetMini { on } => return self.set_mini(app, on),
            Control::ToggleFullscreen => return self.toggle_fullscreen(app),
            Control::Back => {
                if is_fullscreen(app) {
                    return set_fullscreen(app, false);
                }
                return self.close_session();
            }
            Control::Close => return self.close_session(),
            _ => {}
        }
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
                        // sub-add waits for the download; on this (the main) thread that
                        // would freeze both windows, so it runs on its own.
                        let title = format!("{} · {}", sub.label, sub.addon_name);
                        let (url, lang) = (sub.url.clone(), sub.lang.clone());
                        let mpv = mpv.clone();
                        std::thread::spawn(move || {
                            let _ = mpv.command(&["sub-add", &url, "select", &title, &lang]);
                        });
                        Ok(())
                    }
                }
            }
            Control::AdjustSubtitleDelay { seconds } => {
                mpv.command(&["add", "sub-delay", &format!("{seconds:.3}")])
            }
            Control::ResetSubtitleDelay => mpv.set_property("sub-delay", "0"),
            Control::SetSubtitleScale { scale } => mpv.set_property(
                "sub-scale",
                &format!(
                    "{:.2}",
                    scale.clamp(SubtitleStyle::MIN_SCALE, SubtitleStyle::MAX_SCALE)
                ),
            ),
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
            Control::ToggleFullscreen
            | Control::Back
            | Control::Close
            | Control::SetMini { .. } => Ok(()),
        }
    }

    fn toggle_fullscreen(&self, app: &AppHandle) -> Result<(), String> {
        if self.is_mini() {
            self.set_mini(app, false)?;
        }
        set_fullscreen(app, !is_fullscreen(app))
    }

    fn close_session(&self) -> Result<(), String> {
        match self.session.lock().unwrap().as_ref() {
            Some(session) => quit(&session.mpv, &session.closing),
            None => Ok(()),
        }
    }

    /// Whether a player is shutting down (asked to quit, not yet torn down).
    pub fn is_closing(&self) -> bool {
        self.session
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|s| s.closing.load(Ordering::SeqCst))
    }

    /// The progress to save for the playing video, when the app is about to exit.
    pub fn final_report(&self) -> Option<WatchReport> {
        let session = self.session.lock().unwrap();
        let session = session.as_ref()?;
        let report = session
            .watch
            .lock()
            .unwrap()
            .as_mut()
            .and_then(|w| w.report(Instant::now(), false));
        report
    }

    /// The video id playing now, if any.
    pub fn current_video(&self) -> Option<String> {
        let session = self.session.lock().unwrap();
        let session = session.as_ref()?;
        let watch = session.watch.lock().unwrap();
        watch.as_ref().map(|w| w.video_id().to_string())
    }

    /// The length of the playing video in seconds, once known.
    pub fn duration(&self) -> Option<f64> {
        self.state().map(|s| s.duration).filter(|d| *d > 0.0)
    }

    /// Shows skip buttons for `video_id` (or why there are none), unless another video has
    /// started meanwhile.
    pub fn set_skips(
        &self,
        app: &AppHandle,
        video_id: &str,
        skips: Result<Vec<SkipSegment>, String>,
    ) {
        if self.current_video().as_deref() != Some(video_id) {
            return;
        }
        if let Some(session) = self.session.lock().unwrap().as_ref() {
            let mut state = session.state.lock().unwrap();
            match skips {
                Ok(skips) => {
                    state.skips = skips;
                    state.skip_note = None;
                }
                Err(note) => {
                    state.skips.clear();
                    state.skip_note = Some(note);
                }
            }
            let _ = app.emit("player-state", state.clone());
        }
    }

    pub fn set_auto_skip(&self, app: &AppHandle, enabled: bool) {
        if let Some(session) = self.session.lock().unwrap().as_ref() {
            let mut state = session.state.lock().unwrap();
            state.auto_skip = enabled;
            let _ = app.emit("player-state", state.clone());
        }
    }

    /// Sets who receives progress reports. Called once at start-up.
    pub fn set_reporter(&self, reporter: Reporter) {
        *self.reporter.lock().unwrap() = Some(reporter);
    }

    /// Applies a new subtitle style to the open player, after Settings changed.
    pub fn set_subtitle_style(&self, style: &SubtitleStyle) {
        if let Some(session) = self.session.lock().unwrap().as_ref() {
            apply_style(&session.mpv, style);
        }
    }

    /// Loads a subtitle file from this computer and shows it.
    pub fn add_subtitle_file(&self, path: &std::path::Path) -> Result<(), String> {
        let session = self.session.lock().unwrap();
        let session = session.as_ref().ok_or("nothing is playing")?;
        let path = path.to_str().ok_or("the file name cannot be read")?;
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Subtitle file");
        session.mpv.command(&["sub-add", path, "select", name])
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
        let _ = self.close_session();
    }

    /// Keeps the video in its place when the main window's client area is `width` x `height`:
    /// filling it, or in the mini player's corner. Main thread only.
    pub fn resize(&self, width: u32, height: u32) {
        let rect = layout::video_rect(width, height, self.is_mini());
        if let Some(session) = self.session.lock().unwrap().as_ref() {
            session.surface.place(rect);
        }
    }

    pub fn is_mini(&self) -> bool {
        self.mini.load(Ordering::SeqCst)
    }

    /// Moves the video into the mini player or back to full size. Main thread only.
    pub fn set_mini(&self, app: &AppHandle, on: bool) -> Result<(), String> {
        if !self.is_open() {
            self.mini.store(false, Ordering::SeqCst);
            return Ok(());
        }
        if cfg!(target_os = "android") {
            // Android has no mini player: the video always fills the screen.
            return Ok(());
        }
        let main = main_window(app)?;
        if on && is_fullscreen(app) {
            // The mini player belongs in the normal window; leaving fullscreen resizes it.
            set_fullscreen(app, false)?;
        }
        self.mini.store(on, Ordering::SeqCst);
        let size = main.inner_size().map_err(|e| e.to_string())?;
        self.resize(size.width, size.height);
        place_controls(app);
        if let Some(session) = self.session.lock().unwrap().as_ref() {
            let mut state = session.state.lock().unwrap();
            state.mini = on;
            let _ = app.emit("player-state", state.clone());
        }
        let _ = app.emit("player", PlayerEvent::Layout { mini: on });
        // Browsing goes on in the main window; the full-size player takes the keys again.
        if on {
            let _ = main.set_focus();
        } else if let Some(controls) = app.get_webview_window(CONTROLS_WINDOW) {
            let _ = controls.set_focus();
        }
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.session.lock().unwrap().is_some()
    }

    /// Removes the session numbered `id`, if it is still the current one.
    fn take_session(&self, id: u64) -> Option<Session> {
        let mut session = self.session.lock().unwrap();
        if session.as_ref().is_some_and(|s| s.id == id) {
            session.take()
        } else {
            None
        }
    }
}

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("main")
        .ok_or_else(|| "the main window is gone".to_string())
}

/// Whether the main window fills the screen.
#[cfg(desktop)]
fn is_fullscreen(app: &AppHandle) -> bool {
    main_window(app).is_ok_and(|w| w.is_fullscreen().unwrap_or(false))
}

#[cfg(desktop)]
fn set_fullscreen(app: &AppHandle, on: bool) -> Result<(), String> {
    main_window(app)?
        .set_fullscreen(on)
        .map_err(|e| e.to_string())
}

/// On Android the player always fills the screen (the Kotlin side hides the system bars).
#[cfg(mobile)]
fn is_fullscreen(_app: &AppHandle) -> bool {
    false
}

#[cfg(mobile)]
fn set_fullscreen(_app: &AppHandle, _on: bool) -> Result<(), String> {
    Ok(())
}

/// On Android the main page draws the controls itself, once it hears `PlayerEvent::Opened`.
#[cfg(mobile)]
pub fn open_controls(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

/// Opens the transparent controls window over the main window's client area. Must not run on
/// the main thread or in a synchronous command: creating a web view there deadlocks on Windows.
#[cfg(desktop)]
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

/// Puts the controls window exactly over the video: the main window's client area, or the
/// mini player's corner of it (physical pixels).
pub fn place_controls(app: &AppHandle) {
    let (Some(main), Some(controls)) = (
        app.get_webview_window("main"),
        app.get_webview_window(CONTROLS_WINDOW),
    ) else {
        return;
    };
    let mini = app
        .try_state::<crate::AppState>()
        .is_some_and(|state| state.player.is_mini());
    if let (Ok(position), Ok(size)) = (main.inner_position(), main.inner_size()) {
        let rect = layout::video_rect(size.width, size.height, mini);
        let at = tauri::PhysicalPosition::new(position.x + rect.x, position.y + rect.y);
        let _ = controls.set_position(tauri::Position::Physical(at));
        let _ = controls.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
            rect.width,
            rect.height,
        )));
    }
}

#[cfg(windows)]
fn window_handle(window: &WebviewWindow) -> Result<isize, String> {
    window
        .hwnd()
        .map(|hwnd| hwnd.0 as isize)
        .map_err(|e| e.to_string())
}

/// On Android the Kotlin plugin makes its own view; there is no parent handle to pass.
#[cfg(target_os = "android")]
fn window_handle(_window: &WebviewWindow) -> Result<isize, String> {
    Ok(0)
}

#[cfg(not(any(windows, target_os = "android")))]
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
fn follow(app: &AppHandle, mpv: &Mpv, closing: &AtomicBool, action: Action, reason: &str) {
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
                let _ = quit(mpv, closing);
            }
        }
        Action::GiveUp { tried, last_error } => {
            let message = if *tried == 1 {
                format!("The stream would not play ({last_error}).")
            } else {
                format!("None of the {tried} streams would play (last: {last_error}).")
            };
            let _ = app.emit("player", PlayerEvent::Failed { message });
            let _ = quit(mpv, closing);
        }
        Action::Wait => {}
    }
}

/// Forwards mpv events to the UI, switches streams on failure, and removes the video and
/// controls windows once mpv shuts down.
#[allow(clippy::too_many_arguments)]
fn spawn_event_thread(
    app: AppHandle,
    id: u64,
    closing: Arc<AtomicBool>,
    mpv: Arc<Mpv>,
    queue: SharedQueue,
    state: SharedState,
    watch: SharedWatch,
    reporter: Option<Reporter>,
) {
    std::thread::spawn(move || {
        let mut last_sent = Instant::now();
        loop {
            // Wake at least once a second so a stream that never starts can be timed out.
            let event = mpv.wait_event(1.0);
            let now = Instant::now();

            // Progress is saved regularly while playing.
            let paused = state.lock().unwrap().paused;
            let due = watch
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|w| w.due(now, paused));
            if due {
                report(&watch, &reporter, false);
            }

            if let Event::Property { name, value } = &event {
                let mut current = state.lock().unwrap();
                let urgent = current.apply(name, value);
                // Until the current stream has loaded, these still describe the one before.
                let loaded = queue
                    .lock()
                    .unwrap()
                    .as_ref()
                    .is_some_and(PlayQueue::is_loaded);
                if let (mpv::PropertyValue::Double(v), true) = (value, loaded) {
                    if let Some(q) = queue.lock().unwrap().as_mut() {
                        match name.as_str() {
                            "time-pos" => q.on_position(*v),
                            "duration" => q.on_duration(*v),
                            _ => {}
                        }
                    }
                    if let Some(w) = watch.lock().unwrap().as_mut() {
                        match name.as_str() {
                            "time-pos" => w.on_position(*v),
                            "duration" => w.on_duration(*v),
                            _ => {}
                        }
                    }
                }
                if urgent || now.duration_since(last_sent) >= POSITION_INTERVAL {
                    last_sent = now;
                    let _ = app.emit("player-state", current.clone());
                }
                let paused_now = name == "pause" && *value == mpv::PropertyValue::Flag(true);
                drop(current);
                if paused_now {
                    report(&watch, &reporter, false);
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
                Event::CloseRequested => {
                    drop(guard);
                    let _ = quit(&mpv, &closing);
                }
                Event::FileLoaded => {
                    q.on_loaded();
                    if let Some(w) = watch.lock().unwrap().as_mut() {
                        w.on_loaded(q.current_key().map(String::from));
                    }
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
                    follow(
                        &app,
                        &mpv,
                        &closing,
                        action,
                        &format!("Stream failed ({error})"),
                    );
                }
                Event::EndOfFile => {
                    if q.finished_normally() {
                        drop(guard);
                        report(&watch, &reporter, true);
                        let _ = app.emit("player", PlayerEvent::Ended);
                    } else {
                        let action = q.on_end_of_file(now);
                        drop(guard);
                        follow(&app, &mpv, &closing, action, "Stream stopped early");
                    }
                }
                Event::None => {
                    let action = q.on_tick(now);
                    drop(guard);
                    follow(&app, &mpv, &closing, action, "Stream did not start");
                }
                _ => {}
            }
        }
        // Save where playback stopped.
        report(&watch, &reporter, false);
        drop(mpv);
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let player = handle.state::<crate::AppState>();
            // Only this session's own teardown; a newer one is left alone.
            let Some(session) = player.player.take_session(id) else {
                return;
            };
            // Destroying the last Arc ends mpv; the window goes after it stops drawing.
            drop(session.mpv);
            session.surface.destroy();
            player.player.mini.store(false, Ordering::SeqCst);
            if let Some(controls) = handle.get_webview_window(CONTROLS_WINDOW) {
                let _ = controls.destroy();
            }
            let _ = set_fullscreen(&handle, false);
            if let Some(main) = handle.get_webview_window("main") {
                let _ = main.set_focus();
            }
            let _ = handle.emit("player", PlayerEvent::Closed);
            // Torrents nobody is watching now stop downloading.
            let engine = player.torrents.clone();
            tauri::async_runtime::spawn(async move { engine.pause_idle().await });
        });
    });
}
