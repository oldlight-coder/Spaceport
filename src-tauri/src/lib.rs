mod auth;
mod language;
mod player;
mod progress;
mod ranking;
mod secrets;
mod settings;
mod stremio;
mod torrent;
mod watched;

use serde_json::Value;
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

use auth::Session;
use ranking::RankedResults;
use secrets::{KeyringStore, SecretStore};
use settings::{Settings, SettingsStore};
use stremio::addons::{self, Addon, AddonError};
use stremio::api::{StremioApi, ADDON_TIMEOUT, DEFAULT_BASE_URL};
use stremio::catalog::{self, CatalogRef, MetaPreview, CINEMETA_URL};
use stremio::library::{visible_items, LibraryItem};
use stremio::meta::{self, MetaDetails};
use stremio::streams::{self, STREAM_TIMEOUT};

pub(crate) struct AppState {
    api: StremioApi,
    secrets: Box<dyn SecretStore>,
    /// Addon descriptors the app may request resources from: the user's collection as last
    /// read from Stremio. Cinemeta is always allowed as the default source.
    sources: Mutex<Vec<Value>>,
    settings: Mutex<Settings>,
    settings_store: SettingsStore,
    player: player::Player,
    torrents: Arc<torrent::TorrentEngine>,
    /// Positions of every video watched, kept on this computer.
    resume: progress::ResumeStore,
    /// One progress save at a time, so two saves of the same title never race.
    progress_lock: tokio::sync::Mutex<()>,
}

impl AppState {
    fn auth_key(&self) -> Result<String, String> {
        auth::auth_key(self.secrets.as_ref()).map_err(|e| e.to_string())
    }

    async fn addon_collection(&self, key: &str) -> Result<Vec<Value>, String> {
        let collection = self
            .api
            .addon_collection(key)
            .await
            .map_err(|e| e.to_string())?;
        self.set_sources(collection.clone());
        Ok(collection)
    }

    /// The addons to use for resource requests, read from Stremio on first use. Cinemeta is
    /// appended when it is not installed so titles always have a metadata source.
    async fn sources(&self) -> Result<Vec<Value>, String> {
        let cached = self.sources.lock().unwrap().clone();
        let mut sources = if cached.is_empty() {
            let key = self.auth_key()?;
            self.addon_collection(&key).await?
        } else {
            cached
        };
        let has_cinemeta = sources
            .iter()
            .any(|d| d.get("transportUrl").and_then(Value::as_str) == Some(CINEMETA_URL));
        if !has_cinemeta {
            if let Ok(manifest) = self.api.fetch_manifest(CINEMETA_URL).await {
                sources.push(addons::builtin_descriptor(CINEMETA_URL, manifest));
            }
        }
        Ok(sources)
    }

    fn set_sources(&self, sources: Vec<Value>) {
        *self.sources.lock().unwrap() = sources;
    }

    /// Only addons the user installed (or the Cinemeta default) may be contacted.
    fn ensure_source(&self, transport_url: &str) -> Result<(), String> {
        let known = transport_url == CINEMETA_URL
            || self
                .sources
                .lock()
                .unwrap()
                .iter()
                .any(|d| d.get("transportUrl").and_then(Value::as_str) == Some(transport_url));
        if known {
            Ok(())
        } else {
            Err("this addon is not installed".to_string())
        }
    }

    /// Reads the collection from Stremio, applies `change` and writes it back, so edits made
    /// in other Stremio apps since the last read are not lost.
    async fn edit_addons(
        &self,
        change: impl FnOnce(&mut Vec<Value>) -> Result<(), AddonError>,
    ) -> Result<Vec<Addon>, String> {
        let key = self.auth_key()?;
        let mut collection = self.addon_collection(&key).await?;
        change(&mut collection).map_err(|e| e.to_string())?;
        self.api
            .set_addon_collection(&key, &collection)
            .await
            .map_err(|e| e.to_string())?;
        let views = addons::addon_views(&collection);
        self.set_sources(collection);
        Ok(views)
    }
}

#[tauri::command]
async fn auth_restore(state: tauri::State<'_, AppState>) -> Result<Session, String> {
    auth::restore(&state.api, state.secrets.as_ref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn auth_login(
    state: tauri::State<'_, AppState>,
    email: String,
    password: String,
) -> Result<Session, String> {
    auth::login(&state.api, state.secrets.as_ref(), &email, &password)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn auth_logout(state: tauri::State<'_, AppState>) -> Result<Session, String> {
    auth::logout(&state.api, state.secrets.as_ref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn library_list(state: tauri::State<'_, AppState>) -> Result<Vec<LibraryItem>, String> {
    let key = state.auth_key()?;
    let raw = state
        .api
        .library_items(&key)
        .await
        .map_err(|e| e.to_string())?;
    Ok(visible_items(&raw))
}

#[tauri::command]
async fn addons_list(state: tauri::State<'_, AppState>) -> Result<Vec<Addon>, String> {
    let key = state.auth_key()?;
    Ok(addons::addon_views(&state.addon_collection(&key).await?))
}

#[tauri::command]
async fn addons_install(
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<Vec<Addon>, String> {
    let url = addons::normalize_manifest_url(&url).map_err(|e| e.to_string())?;
    let manifest = state
        .api
        .fetch_manifest(&url)
        .await
        .map_err(|e| e.to_string())?;
    addons::validate_manifest(&manifest).map_err(|e| e.to_string())?;
    state
        .edit_addons(|c| {
            addons::install(c, &url, manifest);
            Ok(())
        })
        .await
}

#[tauri::command]
async fn addons_remove(
    state: tauri::State<'_, AppState>,
    transport_url: String,
) -> Result<Vec<Addon>, String> {
    state
        .edit_addons(|c| addons::remove(c, &transport_url))
        .await
}

#[tauri::command]
async fn addons_reorder(
    state: tauri::State<'_, AppState>,
    order: Vec<String>,
) -> Result<Vec<Addon>, String> {
    state.edit_addons(|c| addons::reorder(c, &order)).await
}

/// Browsable catalogs of the installed addons, or Cinemeta's when no installed addon has any.
#[tauri::command]
async fn catalogs_list(state: tauri::State<'_, AppState>) -> Result<Vec<CatalogRef>, String> {
    let key = state.auth_key()?;
    let mut sources = state.addon_collection(&key).await?;
    if !catalog::has_catalog_addon(&sources) {
        let manifest = state
            .api
            .fetch_manifest(CINEMETA_URL)
            .await
            .map_err(|e| e.to_string())?;
        sources.insert(0, addons::builtin_descriptor(CINEMETA_URL, manifest));
    }
    Ok(catalog::browsable_catalogs(&sources))
}

/// Catalogs that accept a search term, from the installed addons or Cinemeta.
#[tauri::command]
async fn search_catalogs(state: tauri::State<'_, AppState>) -> Result<Vec<CatalogRef>, String> {
    let key = state.auth_key()?;
    let mut sources = state.addon_collection(&key).await?;
    if catalog::search_catalogs(&sources).is_empty() {
        let manifest = state
            .api
            .fetch_manifest(CINEMETA_URL)
            .await
            .map_err(|e| e.to_string())?;
        sources.insert(0, addons::builtin_descriptor(CINEMETA_URL, manifest));
    }
    Ok(catalog::search_catalogs(&sources))
}

/// Fetches one page of a catalog from an installed addon.
#[tauri::command]
async fn catalog_fetch(
    state: tauri::State<'_, AppState>,
    transport_url: String,
    kind: String,
    id: String,
    extra: Vec<(String, String)>,
) -> Result<Vec<MetaPreview>, String> {
    state.ensure_source(&transport_url)?;
    let url = catalog::resource_url(&transport_url, "catalog", &kind, &id, &extra)
        .map_err(|e| e.to_string())?;
    let body = state
        .api
        .addon_json(&url, ADDON_TIMEOUT)
        .await
        .map_err(|e| e.to_string())?;
    Ok(catalog::parse_metas(&body))
}

/// Details of one title from the first addon that has them, trying `preferred` (the addon the
/// title was found in) first.
#[tauri::command]
async fn meta_details(
    state: tauri::State<'_, AppState>,
    kind: String,
    id: String,
    preferred: Option<String>,
) -> Result<MetaDetails, String> {
    let sources = state.sources().await?;
    let urls = addons::resource_sources(&sources, "meta", &kind, &id, preferred.as_deref());
    let mut last_error = "none of your addons has details for this title".to_string();
    for transport_url in urls {
        let url = catalog::resource_url(&transport_url, "meta", &kind, &id, &[])
            .map_err(|e| e.to_string())?;
        match state.api.addon_json(&url, ADDON_TIMEOUT).await {
            Ok(body) => {
                if let Some(details) = meta::parse_meta(&body) {
                    return Ok(details);
                }
            }
            Err(e) => last_error = e.to_string(),
        }
    }
    Err(last_error)
}

/// Streams for one video (a movie id or an episode's video id) from every addon that declares
/// `stream` for it, all asked at once with a per-addon timeout, best first.
#[tauri::command]
async fn streams_fetch(
    state: tauri::State<'_, AppState>,
    kind: String,
    video_id: String,
) -> Result<RankedResults, String> {
    let sources = state.sources().await?;
    let urls = addons::resource_sources(&sources, "stream", &kind, &video_id, None);
    let views = addons::addon_views(&sources);
    let targets = urls
        .into_iter()
        .map(|url| {
            let name = views
                .iter()
                .find(|a| a.transport_url == url)
                .map(|a| a.name.clone())
                .unwrap_or_else(|| url.clone());
            (name, url)
        })
        .collect();
    let results =
        streams::fetch_streams(&state.api, targets, &kind, &video_id, STREAM_TIMEOUT).await;
    let settings = state.settings.lock().unwrap().clone();
    Ok(ranking::rank_results(
        results,
        &settings.scoring_rules(),
        &settings.filters,
    ))
}

/// A stream to try, as the UI sends it: a web link, or a torrent for the built-in engine.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayCandidate {
    label: String,
    url: Option<String>,
    torrent: Option<torrent::TorrentSource>,
}

/// Plays the first of the ranked candidates in the embedded player and falls back to the next
/// on failure, then opens the controls window and looks up subtitles from the addons. Async
/// because creating a window from a synchronous command deadlocks on Windows; the video window
/// itself is made on the main thread.
#[tauri::command]
async fn player_open(
    app: tauri::AppHandle,
    candidates: Vec<PlayCandidate>,
    title: String,
    item: progress::TitleInfo,
    video_id: String,
) -> Result<(), String> {
    let kind = item.kind.clone();
    let engine = app.state::<AppState>().torrents.clone();
    let mut playable = Vec::new();
    for candidate in candidates {
        let url = match (candidate.url, candidate.torrent) {
            (Some(url), _) if url.starts_with("https://") || url.starts_with("http://") => url,
            // Torrents play through the local stream server.
            (None, Some(source)) => match engine.register(source).await {
                Ok(url) => url,
                Err(_) => continue,
            },
            _ => continue,
        };
        playable.push(player::Candidate {
            url,
            label: candidate.label,
        });
    }
    if playable.is_empty() {
        return Err("none of these streams can be played".to_string());
    }
    let candidates = playable;
    let settings = app.state::<AppState>().settings.lock().unwrap().clone();
    let prefs = player::PlayerPrefs {
        audio_languages: language::mpv_list(&settings.audio_languages),
        subtitle_languages: language::mpv_list(&settings.subtitle_languages),
        subtitle_style: settings.subtitle_style.clone(),
    };
    let start = resume_position(&app, &item.id, &video_id).await;
    let target = player::WatchTarget {
        title: item,
        video_id: video_id.clone(),
    };
    let (sender, receiver) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let state = handle.state::<AppState>();
        let result = state
            .player
            .open(&handle, candidates, &title, &prefs, target, start);
        let _ = sender.send(result);
    })
    .map_err(|e| e.to_string())?;
    receiver
        .recv()
        .map_err(|_| "the player did not start".to_string())??;
    player::open_controls(&app)?;

    // Subtitles are not needed to start playing, so they arrive when the addons answer.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = handle.state::<AppState>();
        let Ok(sources) = state.sources().await else {
            return;
        };
        let urls = addons::resource_sources(&sources, "subtitles", &kind, &video_id, None);
        let views = addons::addon_views(&sources);
        let targets = urls
            .into_iter()
            .map(|url| {
                let name = views
                    .iter()
                    .find(|a| a.transport_url == url)
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| url.clone());
                (name, url)
            })
            .collect();
        let found =
            stremio::subtitles::fetch_subtitles(&state.api, targets, &kind, &video_id).await;
        let ordered = stremio::subtitles::order_and_label(found, &settings.subtitle_languages);
        state.player.set_addon_subtitles(&handle, ordered);
    });
    Ok(())
}

/// Where to resume a video: the newer of this computer's position and the one in the Stremio
/// library (which may come from another device). Stremio is not waited on for long, so being
/// offline does not hold up playback.
async fn resume_position(app: &tauri::AppHandle, title_id: &str, video_id: &str) -> Option<f64> {
    let state = app.state::<AppState>();
    let local = state.resume.load().get(video_id).copied();
    let remote = async {
        let key = state.auth_key().ok()?;
        let item = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            state.api.library_item(&key, title_id),
        )
        .await
        .ok()?
        .ok()??;
        let s = item.get("state")?;
        if s.get("video_id").and_then(Value::as_str) != Some(video_id) {
            return None;
        }
        let ms = |k: &str| s.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        Some(progress::VideoProgress {
            position: ms("timeOffset") / 1000.0,
            duration: ms("duration") / 1000.0,
            updated: s
                .get("lastWatched")
                .and_then(Value::as_str)
                .and_then(progress::parse_iso_millis)
                .unwrap_or(0),
        })
    }
    .await;
    let newest = match (local, remote) {
        (Some(l), Some(r)) => Some(if r.updated > l.updated { r } else { l }),
        (l, r) => l.or(r),
    }?;
    progress::resume_point(newest.position, newest.duration)
}

/// Saves a progress report on this computer and in the Stremio library item of the title.
async fn save_progress(app: tauri::AppHandle, report: player::WatchReport) {
    let state = app.state::<AppState>();
    let _one_at_a_time = state.progress_lock.lock().await;
    let position = progress::stored_position(report.position, report.duration, report.finished);
    let _ = state.resume.update(
        &report.target.video_id,
        progress::VideoProgress {
            position,
            duration: report.duration,
            updated: progress::now_millis(),
        },
    );
    let Ok(key) = state.auth_key() else {
        return;
    };
    let now = progress::iso_time(std::time::SystemTime::now());
    let title = &report.target.title;
    let mut item = match state.api.library_item(&key, &title.id).await {
        Ok(Some(item)) => item,
        Ok(None) => progress::new_library_item(title, &now),
        // Offline: the local copy is saved; Stremio gets the next report.
        Err(_) => return,
    };
    progress::apply_progress(
        &mut item,
        &report.target.video_id,
        position,
        report.duration,
        report.watched_seconds,
        &now,
    );
    // Watching 90% or more marks the video watched, as the official apps do.
    let mostly_watched = report.duration > 0.0 && report.position >= report.duration * 0.9;
    if report.finished || mostly_watched {
        let videos = if report.target.video_id == title.id {
            Vec::new()
        } else {
            vec![report.target.video_id.clone()]
        };
        if videos.is_empty() || !title.video_ids.is_empty() {
            watched::apply_marks(&mut item, &title.video_ids, &videos, true, &now);
        }
    }
    let _ = state.api.put_library_items(&key, &[item]).await;
}

/// What is marked watched for a title, read from its Stremio library item.
#[tauri::command]
async fn watched_get(
    state: tauri::State<'_, AppState>,
    item: progress::TitleInfo,
) -> Result<watched::WatchedMarks, String> {
    let key = state.auth_key()?;
    let stored = state
        .api
        .library_item(&key, &item.id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(match stored {
        Some(stored) => watched::read_marks(&stored, &item.video_ids),
        None => watched::WatchedMarks {
            title: false,
            videos: Vec::new(),
        },
    })
}

/// Marks videos (or, with `videos` empty, the whole title) watched or unwatched in Stremio.
#[tauri::command]
async fn watched_set(
    state: tauri::State<'_, AppState>,
    item: progress::TitleInfo,
    videos: Vec<String>,
    watched: bool,
) -> Result<watched::WatchedMarks, String> {
    let key = state.auth_key()?;
    let _one_at_a_time = state.progress_lock.lock().await;
    let now = progress::iso_time(std::time::SystemTime::now());
    let mut stored = state
        .api
        .library_item(&key, &item.id)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| progress::new_library_item(&item, &now));
    watched::apply_marks(&mut stored, &item.video_ids, &videos, watched, &now);
    state
        .api
        .put_library_items(&key, std::slice::from_ref(&stored))
        .await
        .map_err(|e| e.to_string())?;
    // A video marked watched starts from the beginning next time here too.
    if watched {
        let cleared: Vec<String> = if videos.is_empty() {
            let mut all = item.video_ids.clone();
            all.push(item.id.clone());
            all
        } else {
            videos
        };
        for video in cleared {
            let _ = state.resume.update(
                &video,
                progress::VideoProgress {
                    position: 0.0,
                    duration: 0.0,
                    updated: progress::now_millis(),
                },
            );
        }
    }
    Ok(watched::read_marks(&stored, &item.video_ids))
}

/// The Continue Watching row: titles started and not finished, from the Stremio library.
#[tauri::command]
async fn continue_watching(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<progress::ContinueItem>, String> {
    let key = state.auth_key()?;
    let items = state
        .api
        .library_items(&key)
        .await
        .map_err(|e| e.to_string())?;
    Ok(progress::continue_watching(&items))
}

/// Saved positions of every video of a title, for progress bars on the details page.
#[tauri::command]
fn title_progress(
    state: tauri::State<'_, AppState>,
    id: String,
) -> std::collections::HashMap<String, progress::VideoProgress> {
    state.resume.for_title(&id)
}

#[tauri::command]
fn player_close(state: tauri::State<'_, AppState>) {
    state.player.close();
}

/// A button or key press in the controls window. A subtitle size change is also saved as the
/// new default.
#[tauri::command]
fn player_control(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    control: player::Control,
) -> Result<(), String> {
    let scale = match &control {
        player::Control::SetSubtitleScale { scale } => Some(*scale),
        _ => None,
    };
    state.player.control(&app, control)?;
    if let Some(scale) = scale {
        let mut settings = state.settings.lock().unwrap().clone();
        settings.subtitle_style.scale = scale.clamp(
            settings::SubtitleStyle::MIN_SCALE,
            settings::SubtitleStyle::MAX_SCALE,
        );
        state
            .settings_store
            .save(&settings)
            .map_err(|e| e.to_string())?;
        *state.settings.lock().unwrap() = settings;
    }
    Ok(())
}

/// Lets the user pick a subtitle file from this computer and shows it in the player.
#[tauri::command]
async fn player_load_subtitle(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;
    let dialog = app
        .dialog()
        .file()
        .set_title("Choose a subtitle file")
        .add_filter(
            "Subtitles",
            &["srt", "ass", "ssa", "vtt", "sub", "idx", "sup", "smi"],
        );
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_file())
        .await
        .map_err(|e| e.to_string())?;
    let Some(file) = picked else {
        return Ok(());
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    app.state::<AppState>().player.add_subtitle_file(&path)
}

/// The playback state for a controls window that has just loaded.
#[tauri::command]
fn player_state(state: tauri::State<'_, AppState>) -> Option<player::PlaybackStateView> {
    state.player.state()
}

#[tauri::command]
fn settings_get(state: tauri::State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

/// Saves new settings to disk and applies them to the next stream ranking.
#[tauri::command]
fn settings_set(state: tauri::State<'_, AppState>, settings: Settings) -> Result<Settings, String> {
    state
        .settings_store
        .save(&settings)
        .map_err(|e| e.to_string())?;
    state.torrents.set_dht(settings.torrent_dht);
    state.torrents.set_cache_limit(settings.torrent_cache_gb);
    state.player.set_subtitle_style(&settings.subtitle_style);
    *state.settings.lock().unwrap() = settings.clone();
    Ok(settings)
}

/// Opens an addon's configure page in the default browser.
#[tauri::command]
async fn addons_configure(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    transport_url: String,
) -> Result<(), String> {
    let key = state.auth_key()?;
    let collection = state.addon_collection(&key).await?;
    let url = addons::configure_url(&collection, &transport_url).map_err(|e| e.to_string())?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let settings_store = SettingsStore::new(&config_dir);
            let settings = settings_store.load();
            let torrent_cache = app.path().app_cache_dir()?.join("torrents");
            let torrents = torrent::TorrentEngine::new(
                torrent_cache,
                settings.torrent_dht,
                settings.torrent_cache_gb,
            );
            app.manage(AppState {
                api: StremioApi::new(DEFAULT_BASE_URL),
                secrets: Box::new(KeyringStore),
                sources: Mutex::new(Vec::new()),
                settings: Mutex::new(settings),
                settings_store,
                player: player::Player::default(),
                torrents,
                resume: progress::ResumeStore::new(&config_dir),
                progress_lock: tokio::sync::Mutex::new(()),
            });
            let handle = app.handle().clone();
            app.state::<AppState>()
                .player
                .set_reporter(Arc::new(move |report| {
                    let handle = handle.clone();
                    tauri::async_runtime::spawn(save_progress(handle, report));
                }));
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            match event {
                tauri::WindowEvent::Resized(size) => {
                    if let Some(state) = window.try_state::<AppState>() {
                        if state.player.is_open() {
                            state.player.resize(size.width, size.height);
                            player::place_controls(window.app_handle());
                        }
                    }
                }
                tauri::WindowEvent::Moved(_) => player::place_controls(window.app_handle()),
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            auth_restore,
            auth_login,
            auth_logout,
            library_list,
            addons_list,
            addons_install,
            addons_remove,
            addons_reorder,
            addons_configure,
            catalogs_list,
            catalog_fetch,
            search_catalogs,
            meta_details,
            streams_fetch,
            settings_get,
            settings_set,
            player_open,
            player_close,
            player_control,
            player_state,
            player_load_subtitle,
            continue_watching,
            title_progress,
            watched_get,
            watched_set
        ])
        .build(tauri::generate_context!())
        .expect("error while building Spaceport")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                // Stop the torrent engine and delete everything it downloaded.
                if let Some(state) = app.try_state::<AppState>() {
                    let engine = state.torrents.clone();
                    tauri::async_runtime::block_on(engine.shutdown());
                }
            }
        });
}
