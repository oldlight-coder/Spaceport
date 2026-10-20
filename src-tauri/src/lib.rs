/// Returns the app version from Cargo.toml so the UI can show it.
#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_version])
        .run(tauri::generate_context!())
        .expect("error while running Spaceport");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_version_matches_package() {
        assert_eq!(app_version(), env!("CARGO_PKG_VERSION"));
    }
}
