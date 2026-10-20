//! Video playback with libmpv, drawn into a child window of the main window.

mod mpv;
mod queue;
mod surface;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use mpv::{Event, Mpv, MpvLib, PropertyValue};
pub use queue::Candidate;
use queue::{Action, PlayQueue};
use surface::Surface;

const OBSERVE_POSITION: u64 = 1;
const OBSERVE_DURATION: u64 = 2;

/// The DLL `pnpm run setup` places in `src-tauri/lib` and the installer puts next to the app.
const LIBRARY_FILE: &str = "libmpv-2.dll";

/// Options applied before mpv starts. Everything that could load code or config from outside
/// the app is off: no user config files, no user scripts, no youtube-dl helper.
const BASE_OPTIONS: &[(&str, &str)] = &[
    ("config", "no"),
    ("load-scripts", "no"),
    ("ytdl", "no"),
    ("terminal", "no"),
    ("hwdec", "auto-safe"),
    ("force-window", "yes"),
    ("idle", "yes"),
    ("keep-open", "no"),
    // mpv's own on-screen controller and key bindings until Spaceport's controls arrive.
    ("osc", "yes"),
    ("input-default-bindings", "yes"),
    ("input-vo-keyboard", "yes"),
    ("network-timeout", "20"),
];

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

/// The candidates of the current title, shared with the event thread.
type SharedQueue = Arc<Mutex<Option<PlayQueue>>>;

struct Session {
    mpv: Arc<Mpv>,
    surface: Surface,
    queue: SharedQueue,
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
    pub fn open(
        &self,
        app: &AppHandle,
        candidates: Vec<Candidate>,
        title: &str,
    ) -> Result<(), String> {
        let (queue, first) =
            PlayQueue::new(candidates, Instant::now()).ok_or("there is no stream to play")?;
        let mut session = self.session.lock().unwrap();
        if let Some(existing) = session.as_ref() {
            existing.mpv.set_property("force-media-title", title)?;
            *existing.queue.lock().unwrap() = Some(queue);
            return apply(&existing.mpv, first);
        }

        let lib = self.lib(app)?;
        let window = app
            .get_webview_window("main")
            .ok_or("the main window is gone")?;
        let parent = window_handle(&window)?;
        let surface = Surface::create(parent)?;
        let wid = surface.id().to_string();
        let mut options = BASE_OPTIONS.to_vec();
        options.push(("wid", &wid));
        let mpv = match Mpv::new(lib, &options) {
            Ok(mpv) => Arc::new(mpv),
            Err(e) => {
                surface.destroy();
                return Err(e);
            }
        };
        // Esc leaves the player, like closing a video anywhere else. Older mpv builds without
        // the keybind command still quit with q.
        let _ = mpv.command(&["keybind", "ESC", "quit"]);
        mpv.observe_double(OBSERVE_POSITION, "time-pos")?;
        mpv.observe_double(OBSERVE_DURATION, "duration")?;
        mpv.set_property("force-media-title", title)?;

        let queue: SharedQueue = Arc::new(Mutex::new(Some(queue)));
        spawn_event_thread(app.clone(), mpv.clone(), queue.clone());
        apply(&mpv, first)?;
        *session = Some(Session {
            mpv,
            surface,
            queue,
        });
        Ok(())
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

    fn take_session(&self) -> Option<Session> {
        self.session.lock().unwrap().take()
    }
}

#[cfg(windows)]
fn window_handle(window: &tauri::WebviewWindow) -> Result<isize, String> {
    window
        .hwnd()
        .map(|hwnd| hwnd.0 as isize)
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
fn window_handle(_window: &tauri::WebviewWindow) -> Result<isize, String> {
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
            let _ = mpv.command(&["show-text", &format!("{reason} - trying {label}"), "4000"]);
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

/// Forwards mpv events to the UI, switches streams on failure, and removes the video window
/// once mpv shuts down.
fn spawn_event_thread(app: AppHandle, mpv: Arc<Mpv>, queue: SharedQueue) {
    std::thread::spawn(move || {
        loop {
            // Wake at least once a second so a stream that never starts can be timed out.
            let event = mpv.wait_event(1.0);
            let mut guard = queue.lock().unwrap();
            let Some(q) = guard.as_mut() else {
                if event == Event::Shutdown {
                    break;
                }
                continue;
            };
            let now = Instant::now();
            match event {
                Event::Shutdown => break,
                Event::FileLoaded => {
                    q.on_loaded();
                    let label = q.current_label().to_string();
                    let _ = app.emit("player", PlayerEvent::Loaded { label });
                }
                Event::Property {
                    name,
                    value: PropertyValue::Double(v),
                } => match name.as_str() {
                    "time-pos" => q.on_position(v),
                    "duration" => q.on_duration(v),
                    _ => {}
                },
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
            let _ = handle.emit("player", PlayerEvent::Closed);
        });
    });
}
