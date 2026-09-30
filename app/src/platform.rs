use std::sync::mpsc::Sender;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hotkey {
    Pause,
    Record,
    Toggle,
}

pub enum Instance {
    Only(InstanceGuard),
    Quit,
}

#[cfg(windows)]
mod win {
    use super::*;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::System::Threading::*;
    use windows_sys::Win32::UI::Input::Ime::ImmAssociateContextEx;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    pub fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub struct InstanceGuard(HANDLE);
    impl Drop for InstanceGuard {
        fn drop(&mut self) {
            unsafe {
                ReleaseMutex(self.0);
                CloseHandle(self.0);
            }
        }
    }

    pub fn instance_suffix() -> String {
        std::env::var("EXPOVERLAY_INSTANCE").unwrap_or_default().chars().filter(|c| c.is_alphanumeric() || *c == '_').collect()
    }

    fn quit_event_name() -> String {
        format!("Local\\ExpOverlayArtale_Quit{}", instance_suffix())
    }

    pub fn message_box(text: &str, flags: u32) -> i32 {
        unsafe { MessageBoxW(std::ptr::null_mut(), wide(text).as_ptr(), wide("經驗收益計算器").as_ptr(), flags | MB_TOPMOST) }
    }

    pub fn single_instance() -> Instance {
        unsafe {
            let name = wide(&format!("Local\\ExpOverlayArtale{}", instance_suffix()));
            let m = CreateMutexW(std::ptr::null(), 1, name.as_ptr());
            if GetLastError() != ERROR_ALREADY_EXISTS {
                return Instance::Only(InstanceGuard(m));
            }
            let ans = message_box(
                "經驗收益計算器已經開著了。\n\n要關掉舊的、換成重新開一個嗎？\n（舊的進度會先存起來，新的會接著算）\n\n按「否」維持原本的。找不到浮窗的話，按 Ctrl+F12 顯示／隱藏。",
                MB_YESNO | MB_ICONQUESTION,
            );
            if ans != IDYES {
                CloseHandle(m);
                return Instance::Quit;
            }
            let r = WaitForSingleObject(m, 0);
            if r != WAIT_OBJECT_0 && r != WAIT_ABANDONED {
                let evt = OpenEventW(EVENT_MODIFY_STATE, 0, wide(&quit_event_name()).as_ptr());
                if !evt.is_null() {
                    SetEvent(evt);
                    CloseHandle(evt);
                }
                let r = WaitForSingleObject(m, if evt.is_null() { 1500 } else { 10000 });
                if r != WAIT_OBJECT_0 && r != WAIT_ABANDONED {
                    let msg = if evt.is_null() {
                        "舊版的浮窗沒辦法自動關閉。\n\n請按舊浮窗右上角的 ✕ 關掉（會存紀錄），再開一次。"
                    } else {
                        "舊的浮窗沒有關掉。\n\n請按它右上角的 ✕，再開一次。"
                    };
                    message_box(msg, MB_ICONWARNING);
                    CloseHandle(m);
                    return Instance::Quit;
                }
            }
            Instance::Only(InstanceGuard(m))
        }
    }

    pub fn watch_quit(f: impl Fn() + Send + 'static) {
        std::thread::spawn(move || unsafe {
            let evt = CreateEventW(std::ptr::null(), 0, 0, wide(&quit_event_name()).as_ptr());
            if evt.is_null() {
                return;
            }
            loop {
                if WaitForSingleObject(evt, INFINITE) == WAIT_OBJECT_0 {
                    f();
                }
            }
        });
    }

    pub fn hotkeys(tx: Sender<Hotkey>, failed: Sender<Vec<&'static str>>, wake: impl Fn() + Send + 'static) {
        std::thread::spawn(move || unsafe {
            let keys = [(1, VK_F10, Hotkey::Pause, "Ctrl+F10"), (2, VK_F11, Hotkey::Record, "Ctrl+F11"), (3, VK_F12, Hotkey::Toggle, "Ctrl+F12")];
            let mut bad = vec![];
            for (id, vk, _, name) in keys {
                if RegisterHotKey(std::ptr::null_mut(), id, MOD_CONTROL | MOD_NOREPEAT, vk as u32) == 0 {
                    bad.push(name);
                }
            }
            if !bad.is_empty() {
                let _ = failed.send(bad);
                wake();
            }
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                if msg.message == WM_HOTKEY {
                    if let Some(k) = keys.iter().find(|k| k.0 as usize == msg.wParam) {
                        let _ = tx.send(k.2);
                        wake();
                    }
                }
            }
        });
    }

    pub fn no_activate(hwnd: isize) {
        unsafe {
            let h = hwnd as HWND;
            let ex = GetWindowLongW(h, GWL_EXSTYLE);
            SetWindowLongW(h, GWL_EXSTYLE, ex | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as i32);
        }
    }

    pub fn set_alpha(hwnd: isize, alpha: f32) {
        use std::sync::Mutex;
        static LAST: Mutex<Option<(isize, u8)>> = Mutex::new(None);
        let a = (alpha.clamp(0.1, 1.0) * 255.0).round() as u8;
        let mut last = LAST.lock().unwrap();
        if *last == Some((hwnd, a)) {
            return;
        }
        *last = Some((hwnd, a));
        unsafe {
            let h = hwnd as HWND;
            let ex = GetWindowLongW(h, GWL_EXSTYLE);
            SetWindowLongW(h, GWL_EXSTYLE, ex | WS_EX_LAYERED as i32);
            SetLayeredWindowAttributes(h, 0, a, LWA_ALPHA);
        }
    }

    pub fn dpi_aware() {
        unsafe {
            windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }

    pub fn dll_dir(dir: &std::path::Path) {
        use std::os::windows::ffi::OsStrExt;
        let w: Vec<u16> = dir.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        unsafe {
            windows_sys::Win32::System::LibraryLoader::SetDllDirectoryW(w.as_ptr());
        }
    }

    pub fn keep_top(hwnd: isize) {
        unsafe {
            SetWindowPos(hwnd as HWND, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }

    pub fn show_quiet(hwnd: isize, show: bool) {
        unsafe {
            ShowWindow(hwnd as HWND, if show { SW_SHOWNOACTIVATE } else { SW_HIDE });
        }
    }

    pub fn move_quiet(hwnd: isize, x: i32, y: i32) {
        unsafe {
            SetWindowPos(hwnd as HWND, std::ptr::null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }

    pub fn cursor_pos() -> (i32, i32) {
        let mut p = POINT { x: 0, y: 0 };
        unsafe {
            GetCursorPos(&mut p);
        }
        (p.x, p.y)
    }

    pub fn window_pos(hwnd: isize) -> (i32, i32) {
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe {
            GetWindowRect(hwnd as HWND, &mut r);
        }
        (r.left, r.top)
    }

    pub fn find_window(title: &str) -> Option<isize> {
        let h = unsafe { FindWindowW(std::ptr::null(), wide(title).as_ptr()) };
        if h.is_null() { None } else { Some(h as isize) }
    }

    pub fn ime_off(hwnd: isize) {
        unsafe {
            ImmAssociateContextEx(hwnd as HWND, std::ptr::null_mut(), 0);
        }
    }

    pub fn force_focus(hwnd: isize) {
        unsafe {
            let h = hwnd as HWND;
            let fg = GetForegroundWindow();
            if !fg.is_null() && fg != h {
                let me = GetCurrentThreadId();
                let other = GetWindowThreadProcessId(fg, std::ptr::null_mut());
                let attached = other != 0 && other != me && AttachThreadInput(me, other, 1) != 0;
                BringWindowToTop(h);
                SetForegroundWindow(h);
                if attached {
                    AttachThreadInput(me, other, 0);
                }
            }
        }
    }

    pub fn foreground() -> isize {
        unsafe { GetForegroundWindow() as isize }
    }

    pub fn set_foreground(hwnd: isize) {
        unsafe {
            SetForegroundWindow(hwnd as HWND);
        }
    }

    pub fn on_screen(x: i32, y: i32) -> bool {
        use windows_sys::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONULL};
        unsafe { !MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONULL).is_null() }
    }

    pub fn virtual_screen() -> (i32, i32, i32, i32) {
        unsafe {
            (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            )
        }
    }

    pub fn primary_screen() -> (i32, i32, i32, i32) {
        unsafe { (0, 0, GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) }
    }

    pub fn below_normal_priority() {
        unsafe {
            SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
        }
    }
}

#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
mod other {
    use super::*;
    pub struct InstanceGuard;
    pub fn single_instance() -> Instance {
        Instance::Only(InstanceGuard)
    }
    pub fn watch_quit(_f: impl Fn() + Send + 'static) {}
    pub fn hotkeys(_tx: Sender<Hotkey>, _failed: Sender<Vec<&'static str>>, _wake: impl Fn() + Send + 'static) {}
    pub fn message_box(_text: &str, _flags: u32) -> i32 {
        0
    }
    pub fn no_activate(_hwnd: isize) {}
    pub fn set_alpha(_hwnd: isize, _alpha: f32) {}
    pub fn dpi_aware() {}
    pub fn dll_dir(_dir: &std::path::Path) {}
    pub fn keep_top(_hwnd: isize) {}
    pub fn show_quiet(_hwnd: isize, _show: bool) {}
    pub fn move_quiet(_hwnd: isize, _x: i32, _y: i32) {}
    pub fn cursor_pos() -> (i32, i32) {
        (0, 0)
    }
    pub fn window_pos(_hwnd: isize) -> (i32, i32) {
        (0, 0)
    }
    pub fn find_window(_title: &str) -> Option<isize> {
        None
    }
    pub fn ime_off(_hwnd: isize) {}
    pub fn force_focus(_hwnd: isize) {}
    pub fn foreground() -> isize {
        0
    }
    pub fn set_foreground(_hwnd: isize) {}
    pub fn on_screen(_x: i32, _y: i32) -> bool {
        true
    }
    pub fn virtual_screen() -> (i32, i32, i32, i32) {
        (0, 0, 1920, 1080)
    }
    pub fn primary_screen() -> (i32, i32, i32, i32) {
        (0, 0, 1920, 1080)
    }
    pub fn below_normal_priority() {}
}

#[cfg(not(windows))]
pub use other::*;
