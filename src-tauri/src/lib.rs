mod auth;
mod secrets;
mod stremio;

use auth::Session;
use secrets::{KeyringStore, SecretStore};
use stremio::addons::Addon;
use stremio::api::{StremioApi, DEFAULT_BASE_URL};
use stremio::library::{visible_items, LibraryItem};

struct AppState {
    api: StremioApi,
    secrets: Box<dyn SecretStore>,
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
    let key = auth::auth_key(state.secrets.as_ref()).map_err(|e| e.to_string())?;
    let raw = state
        .api
        .library_items(&key)
        .await
        .map_err(|e| e.to_string())?;
    Ok(visible_items(raw))
}

#[tauri::command]
async fn addons_list(state: tauri::State<'_, AppState>) -> Result<Vec<Addon>, String> {
    let key = auth::auth_key(state.secrets.as_ref()).map_err(|e| e.to_string())?;
    let raw = state
        .api
        .addon_collection(&key)
        .await
        .map_err(|e| e.to_string())?;
    Ok(raw.into_iter().map(Addon::from).collect())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            api: StremioApi::new(DEFAULT_BASE_URL),
            secrets: Box::new(KeyringStore),
        })
        .invoke_handler(tauri::generate_handler![
            auth_restore,
            auth_login,
            auth_logout,
            library_list,
            addons_list
        ])
        .run(tauri::generate_context!())
        .expect("error while running Spaceport");
}
