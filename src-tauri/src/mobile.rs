//! Spaceport's own plugin for the Android-only native pieces. Its Kotlin side is
//! `gen/android/app/src/main/java/com/spaceport/client/SpaceportPlugin.kt`; on other platforms
//! the plugin does nothing.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{AppHandle, Runtime};

/// The handle Rust calls the Kotlin side through.
#[cfg(target_os = "android")]
pub struct Native<R: Runtime>(pub tauri::plugin::PluginHandle<R>);

/// Whether the device is on Wi-Fi or Ethernet rather than mobile data. When that cannot be
/// told, it counts as mobile data. Always true off Android.
pub async fn on_wifi<R: Runtime>(app: &AppHandle<R>) -> bool {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        #[derive(serde::Deserialize)]
        struct Network {
            wifi: bool,
        }
        let Some(native) = app.try_state::<Native<R>>() else {
            return false;
        };
        native
            .0
            .run_mobile_plugin_async::<Network>("networkStatus", serde_json::json!({}))
            .await
            .is_ok_and(|n| n.wifi)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        true
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("spaceport")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                let handle =
                    _api.register_android_plugin("com.spaceport.client", "SpaceportPlugin")?;
                _app.manage(Native(handle));
            }
            Ok(())
        })
        .build()
}
