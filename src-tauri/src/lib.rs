mod auth;
mod secrets;
mod stremio;

use serde_json::Value;
use tauri_plugin_opener::OpenerExt;

use auth::Session;
use secrets::{KeyringStore, SecretStore};
use stremio::addons::{self, Addon, AddonError};
use stremio::api::{StremioApi, DEFAULT_BASE_URL};
use stremio::library::{visible_items, LibraryItem};

struct AppState {
    api: StremioApi,
    secrets: Box<dyn SecretStore>,
}

impl AppState {
    fn auth_key(&self) -> Result<String, String> {
        auth::auth_key(self.secrets.as_ref()).map_err(|e| e.to_string())
    }

    async fn addon_collection(&self, key: &str) -> Result<Vec<Value>, String> {
        self.api
            .addon_collection(key)
            .await
            .map_err(|e| e.to_string())
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
        Ok(addons::addon_views(&collection))
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
            addons_configure
        ])
        .run(tauri::generate_context!())
        .expect("error while running Spaceport");
}
