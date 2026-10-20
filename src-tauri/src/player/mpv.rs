//! A small binding to libmpv's C client API, loaded from the DLL shipped with the app.
//!
//! Loading at runtime (instead of linking) keeps the build free of import libraries and lets
//! non-Windows dev builds compile without mpv. Only the functions Spaceport uses are bound.
//! Reference: mpv's `client.h`.

use std::ffi::{c_char, c_double, c_int, c_void, CStr, CString};
use std::path::Path;
use std::sync::Arc;

use libloading::Library;

#[repr(C)]
pub struct RawHandle {
    _private: [u8; 0],
}

#[repr(C)]
pub struct RawEvent {
    pub event_id: c_int,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct RawEndFile {
    pub reason: c_int,
    pub error: c_int,
    pub playlist_entry_id: i64,
    pub playlist_insert_id: i64,
    pub playlist_insert_num_entries: c_int,
}

#[repr(C)]
pub struct RawProperty {
    pub name: *const c_char,
    pub format: c_int,
    pub data: *mut c_void,
}

// Event ids from client.h.
pub const EVENT_NONE: c_int = 0;
pub const EVENT_SHUTDOWN: c_int = 1;
pub const EVENT_END_FILE: c_int = 7;
pub const EVENT_FILE_LOADED: c_int = 8;
pub const EVENT_PROPERTY_CHANGE: c_int = 22;

// End-of-file reasons.
pub const END_EOF: c_int = 0;
pub const END_ERROR: c_int = 4;

// Property formats.
pub const FORMAT_FLAG: c_int = 3;
pub const FORMAT_DOUBLE: c_int = 5;

type CreateFn = unsafe extern "C" fn() -> *mut RawHandle;
type InitializeFn = unsafe extern "C" fn(*mut RawHandle) -> c_int;
type DestroyFn = unsafe extern "C" fn(*mut RawHandle);
type SetOptionStringFn =
    unsafe extern "C" fn(*mut RawHandle, *const c_char, *const c_char) -> c_int;
type CommandFn = unsafe extern "C" fn(*mut RawHandle, *mut *const c_char) -> c_int;
type SetPropertyStringFn =
    unsafe extern "C" fn(*mut RawHandle, *const c_char, *const c_char) -> c_int;
type WaitEventFn = unsafe extern "C" fn(*mut RawHandle, c_double) -> *mut RawEvent;
type ErrorStringFn = unsafe extern "C" fn(c_int) -> *const c_char;

/// The loaded library and the functions taken from it.
pub struct MpvLib {
    create: CreateFn,
    initialize: InitializeFn,
    terminate_destroy: DestroyFn,
    set_option_string: SetOptionStringFn,
    command: CommandFn,
    set_property_string: SetPropertyStringFn,
    wait_event: WaitEventFn,
    error_string: ErrorStringFn,
    // Kept last so the function pointers above never outlive the library.
    _lib: Library,
}

impl MpvLib {
    /// Loads libmpv from `path`. The file is the one bundled at build time, never downloaded.
    pub fn load(path: &Path) -> Result<Arc<Self>, String> {
        // SAFETY: loading libmpv runs its initialisers, which have no special requirements.
        let lib = unsafe { Library::new(path) }
            .map_err(|e| format!("could not load the player library: {e}"))?;
        macro_rules! sym {
            ($name:literal) => {
                // SAFETY: the type matches the declaration in mpv's client.h.
                *unsafe { lib.get(concat!($name, "\0").as_bytes()) }
                    .map_err(|e| format!("the player library lacks {}: {e}", $name))?
            };
        }
        Ok(Arc::new(Self {
            create: sym!("mpv_create"),
            initialize: sym!("mpv_initialize"),
            terminate_destroy: sym!("mpv_terminate_destroy"),
            set_option_string: sym!("mpv_set_option_string"),
            command: sym!("mpv_command"),
            set_property_string: sym!("mpv_set_property_string"),
            wait_event: sym!("mpv_wait_event"),
            error_string: sym!("mpv_error_string"),
            _lib: lib,
        }))
    }

    pub fn error_text(&self, code: c_int) -> String {
        // SAFETY: mpv_error_string returns a static string for any code.
        unsafe { CStr::from_ptr((self.error_string)(code)) }
            .to_string_lossy()
            .into_owned()
    }
}

fn cstring(value: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| "text contains a NUL character".to_string())
}

/// One mpv player instance. mpv's client API is thread-safe, so it can be shared.
pub struct Mpv {
    lib: Arc<MpvLib>,
    handle: *mut RawHandle,
}

// SAFETY: every mpv client API function may be called from any thread (client.h).
unsafe impl Send for Mpv {}
unsafe impl Sync for Mpv {}

impl Mpv {
    /// Creates an instance, applies `options` before start-up and initialises it.
    pub fn new(lib: Arc<MpvLib>, options: &[(&str, &str)]) -> Result<Self, String> {
        // SAFETY: plain constructor call.
        let handle = unsafe { (lib.create)() };
        if handle.is_null() {
            return Err("could not start the player".to_string());
        }
        let mpv = Self { lib, handle };
        for (name, value) in options {
            let (n, v) = (cstring(name)?, cstring(value)?);
            // SAFETY: valid handle and NUL-terminated strings.
            let code = unsafe { (mpv.lib.set_option_string)(mpv.handle, n.as_ptr(), v.as_ptr()) };
            if code < 0 {
                return Err(format!(
                    "player option {name}: {}",
                    mpv.lib.error_text(code)
                ));
            }
        }
        // SAFETY: valid handle, called once.
        let code = unsafe { (mpv.lib.initialize)(mpv.handle) };
        if code < 0 {
            return Err(format!(
                "could not start the player: {}",
                mpv.lib.error_text(code)
            ));
        }
        Ok(mpv)
    }

    fn check(&self, code: c_int) -> Result<(), String> {
        if code < 0 {
            Err(self.lib.error_text(code))
        } else {
            Ok(())
        }
    }

    pub fn command(&self, args: &[&str]) -> Result<(), String> {
        let owned: Vec<CString> = args.iter().map(|a| cstring(a)).collect::<Result<_, _>>()?;
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|a| a.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        // SAFETY: NULL-terminated array of valid strings that outlive the call.
        self.check(unsafe { (self.lib.command)(self.handle, ptrs.as_mut_ptr()) })
    }

    pub fn set_property(&self, name: &str, value: &str) -> Result<(), String> {
        let (n, v) = (cstring(name)?, cstring(value)?);
        // SAFETY: valid handle and strings.
        self.check(unsafe { (self.lib.set_property_string)(self.handle, n.as_ptr(), v.as_ptr()) })
    }

    /// Blocks up to `timeout` seconds (negative waits forever) for the next event.
    pub fn wait_event(&self, timeout: f64) -> Event {
        // SAFETY: the event stays valid until the next wait_event call on this handle, and is
        // fully converted to owned data before returning.
        unsafe {
            let raw = &*(self.lib.wait_event)(self.handle, timeout);
            match raw.event_id {
                EVENT_SHUTDOWN => Event::Shutdown,
                EVENT_FILE_LOADED => Event::FileLoaded,
                EVENT_END_FILE => {
                    let end = &*(raw.data as *const RawEndFile);
                    match end.reason {
                        END_EOF => Event::EndOfFile,
                        END_ERROR => Event::Failed(self.lib.error_text(end.error)),
                        _ => Event::Stopped,
                    }
                }
                EVENT_PROPERTY_CHANGE => {
                    let prop = &*(raw.data as *const RawProperty);
                    let name = CStr::from_ptr(prop.name).to_string_lossy().into_owned();
                    let value = if prop.data.is_null() {
                        PropertyValue::None
                    } else {
                        match prop.format {
                            FORMAT_FLAG => PropertyValue::Flag(*(prop.data as *const c_int) != 0),
                            FORMAT_DOUBLE => PropertyValue::Double(*(prop.data as *const f64)),
                            _ => PropertyValue::None,
                        }
                    };
                    Event::Property { name, value }
                }
                EVENT_NONE => Event::None,
                _ => Event::Other,
            }
        }
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        // SAFETY: the handle is valid and destroyed exactly once, here.
        unsafe { (self.lib.terminate_destroy)(self.handle) }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    None,
    Flag(bool),
    Double(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    None,
    Shutdown,
    FileLoaded,
    EndOfFile,
    Stopped,
    Failed(String),
    Property { name: String, value: PropertyValue },
    Other,
}
