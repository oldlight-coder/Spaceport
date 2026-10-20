mod auth;
mod secrets;
mod stremio;

use serde_json::Value;
use std::sync::Mutex;
use tauri_plugin_opener::OpenerExt;

use auth::Session;
use secrets::{KeyringStore, SecretStore};
use stremio::addons::{self, Addon, AddonError};
use stremio::api::{StremioApi, ADDON_TIMEOUT, DEFAULT_BASE_URL};
use stremio::catalog::{self, CatalogRef, MetaPreview, CINEMETA_URL};
use stremio::library::{visible_items, LibraryItem};
use stremio::meta::{self, MetaDetails};

struct AppState {
    api: StremioApi,
    secrets: Box<dyn SecretStore>,
    /// Addon descriptors the app may request resources from: the user's collection as last
    /// read from Stremio. Cinemeta is always allowed as the default source.
    sources: Mutex<Vec<Value>>,
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
    Ok(visible_items(raw))
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
        .manage(AppState {
            api: StremioApi::new(DEFAULT_BASE_URL),
            secrets: Box::new(KeyringStore),
            sources: Mutex::new(Vec::new()),
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
            meta_details
        ])
        .run(tauri::generate_context!())
        .expect("error while running Spaceport");
}
