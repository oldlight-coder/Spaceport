//! Video playback with libmpv, drawn into a child window of the main window.

mod mpv;
mod surface;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use mpv::{Event, Mpv, MpvLib};
use surface::Surface;

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
    Loaded,
    Ended,
    Failed { message: String },
    Closed,
}

struct Session {
    mpv: Arc<Mpv>,
    surface: Surface,
}

#[derive(Default)]
pub struct Player {
    lib: OnceLock<Result<Arc<MpvLib>, String>>,
    session: Mutex<Option<Session>>,
}

fn library_path(app: &AppHandle) -> PathBuf {
    let bundled = app.path().resource_dir().map(|dir| dir.join(LIBRARY_FILE));
    match bundled {
        Ok(path) if path.exists() => path,
        _ => std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join(LIBRARY_FILE)))
            .unwrap_or_else(|| PathBuf::from(LIBRARY_FILE)),
    }
}

impl Player {
    fn lib(&self, app: &AppHandle) -> Result<Arc<MpvLib>, String> {
        self.lib
            .get_or_init(|| MpvLib::load(&library_path(app)))
            .clone()
    }

    /// Starts playing `url`, reusing the open player if there is one. Main thread only.
    pub fn open(&self, app: &AppHandle, url: &str, title: &str) -> Result<(), String> {
        let mut session = self.session.lock().unwrap();
        if let Some(existing) = session.as_ref() {
            existing.mpv.set_property("force-media-title", title)?;
            return existing.mpv.command(&["loadfile", url, "replace"]);
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
        mpv.set_property("force-media-title", title)?;
        mpv.command(&["loadfile", url, "replace"])?;

        spawn_event_thread(app.clone(), mpv.clone());
        *session = Some(Session { mpv, surface });
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

/// Forwards mpv events to the UI until mpv shuts down, then removes the video window.
fn spawn_event_thread(app: AppHandle, mpv: Arc<Mpv>) {
    std::thread::spawn(move || {
        loop {
            let event = match mpv.wait_event(-1.0) {
                Event::FileLoaded => Some(PlayerEvent::Loaded),
                Event::EndOfFile => Some(PlayerEvent::Ended),
                Event::Failed(message) => Some(PlayerEvent::Failed { message }),
                Event::Shutdown => break,
                _ => None,
            };
            if let Some(event) = event {
                let _ = app.emit("player", event);
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
