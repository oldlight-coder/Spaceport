//! The native child window mpv draws video into, placed over the web view.
//!
//! Must be created, resized and destroyed on the main (UI) thread.

#[cfg(windows)]
mod imp {
    use std::sync::OnceLock;

    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{GetStockObject, BLACK_BRUSH, HBRUSH};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, LoadCursorW, RegisterClassW,
        SetWindowPos, HWND_TOP, IDC_ARROW, SWP_SHOWWINDOW, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN,
        WS_CLIPSIBLINGS, WS_VISIBLE,
    };

    fn class_name() -> &'static [u16] {
        static NAME: OnceLock<Vec<u16>> = OnceLock::new();
        NAME.get_or_init(|| "SpaceportVideo\0".encode_utf16().collect())
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    fn register_class() -> Result<(), String> {
        static REGISTERED: OnceLock<bool> = OnceLock::new();
        let ok = *REGISTERED.get_or_init(|| {
            // SAFETY: plain Win32 class registration with a static class name.
            unsafe {
                let class = WNDCLASSW {
                    style: 0,
                    lpfnWndProc: Some(window_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: GetModuleHandleW(std::ptr::null()),
                    hIcon: std::ptr::null_mut(),
                    hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                    hbrBackground: GetStockObject(BLACK_BRUSH) as HBRUSH,
                    lpszMenuName: std::ptr::null(),
                    lpszClassName: class_name().as_ptr(),
                };
                RegisterClassW(&class) != 0
            }
        });
        if ok {
            Ok(())
        } else {
            Err("could not create the video window".to_string())
        }
    }

    pub struct Surface {
        hwnd: isize,
    }

    impl Surface {
        /// Creates a black child window filling `parent`'s client area, on top of the web view.
        pub fn create(parent: isize) -> Result<Self, String> {
            register_class()?;
            // SAFETY: `parent` is the live main window; all pointers are valid for the call.
            unsafe {
                let parent = parent as HWND;
                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(parent, &mut rect);
                let hwnd = CreateWindowExW(
                    0,
                    class_name().as_ptr(),
                    std::ptr::null(),
                    WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                    0,
                    0,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    parent,
                    std::ptr::null_mut(),
                    GetModuleHandleW(std::ptr::null()),
                    std::ptr::null(),
                );
                if hwnd.is_null() {
                    return Err("could not create the video window".to_string());
                }
                SetWindowPos(
                    hwnd,
                    HWND_TOP,
                    0,
                    0,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_SHOWWINDOW,
                );
                SetFocus(hwnd);
                Ok(Self {
                    hwnd: hwnd as isize,
                })
            }
        }

        /// The window id mpv's `wid` option expects.
        pub fn id(&self) -> i64 {
            self.hwnd as i64
        }

        pub fn resize(&self, width: u32, height: u32) {
            // SAFETY: the window is alive until `destroy`.
            unsafe {
                SetWindowPos(
                    self.hwnd as HWND,
                    HWND_TOP,
                    0,
                    0,
                    width as i32,
                    height as i32,
                    SWP_SHOWWINDOW,
                );
            }
        }

        pub fn destroy(self) {
            // SAFETY: destroyed once, on the thread that created it.
            unsafe {
                DestroyWindow(self.hwnd as HWND);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    /// Non-Windows builds have no embedded player; they exist for development and tests only.
    pub struct Surface;

    impl Surface {
        pub fn create(_parent: isize) -> Result<Self, String> {
            Err("the player is only available on Windows".to_string())
        }

        pub fn id(&self) -> i64 {
            0
        }

        pub fn resize(&self, _width: u32, _height: u32) {}

        pub fn destroy(self) {}
    }
}

pub use imp::Surface;
