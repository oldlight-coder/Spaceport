//! Spaceport's own plugin for the Android-only native pieces. Its Kotlin side is
//! `gen/android/app/src/main/java/com/spaceport/client/SpaceportPlugin.kt`; on other platforms
//! the plugin does nothing.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;

/// The handle Rust calls the Kotlin side through.
#[cfg(target_os = "android")]
pub struct Native<R: Runtime>(pub tauri::plugin::PluginHandle<R>);

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
