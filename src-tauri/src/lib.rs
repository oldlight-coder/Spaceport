mod auth;
mod secrets;
mod stremio;

use auth::Session;
use secrets::{KeyringStore, SecretStore};
use stremio::api::{StremioApi, DEFAULT_BASE_URL};

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
            auth_logout
        ])
        .run(tauri::generate_context!())
        .expect("error while running Spaceport");
}
