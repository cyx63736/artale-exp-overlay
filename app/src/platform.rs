use std::sync::mpsc::Sender;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hotkey {
    Pause,
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

    pub const HOTKEY_NAMES: [&str; 2] = ["Ctrl+F10", "Ctrl+F12"];

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
            let ans = if std::env::args().any(|a| a == "--replace") {
                IDYES
            } else {
                message_box(
                    "經驗收益計算器已經開著了。\n\n要關掉舊的、換成重新開一個嗎？\n（舊的進度會先存起來，新的會接著算）\n\n按「否」維持原本的。找不到浮窗的話，按 Ctrl+F12 顯示／隱藏。",
                    MB_YESNO | MB_ICONQUESTION,
                )
            };
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
            let keys = [(1, VK_F10, Hotkey::Pause, "Ctrl+F10"), (3, VK_F12, Hotkey::Toggle, "Ctrl+F12")];
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

    pub fn round_corners(hwnd: isize, w: u32, h: u32, radius: f32, scale: f64) {
        use windows_sys::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
        let d = (2.0 * radius as f64 * scale).round() as i32;
        unsafe {
            let rgn = CreateRoundRectRgn(0, 0, w as i32 + 1, h as i32 + 1, d, d);
            if !rgn.is_null() {
                SetWindowRgn(hwnd as HWND, rgn, 1);
            }
        }
    }

    pub fn hint_window(hwnd: isize, frame: (i32, i32, i32, i32), hole: (i32, i32, i32, i32), tab: (i32, i32, i32, i32), radius: i32) {
        use windows_sys::Win32::Graphics::Gdi::{CombineRgn, CreateRoundRectRgn, DeleteObject, SetWindowRgn, RGN_DIFF, RGN_OR};
        let round = |r: (i32, i32, i32, i32), rad: i32| unsafe { CreateRoundRectRgn(r.0, r.1, r.0 + r.2 + 1, r.1 + r.3 + 1, 2 * rad, 2 * rad) };
        let bw = hole.0 - frame.0;
        unsafe {
            let h = hwnd as HWND;
            let ex = GetWindowLongW(h, GWL_EXSTYLE);
            SetWindowLongW(h, GWL_EXSTYLE, ex | (WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as i32);
            SetLayeredWindowAttributes(h, 0, 255, LWA_ALPHA);
            SetWindowDisplayAffinity(h, WDA_EXCLUDEFROMCAPTURE);
            let (rgn, cut, t) = (round(frame, radius), round(hole, (radius - bw).max(0)), round(tab, radius));
            CombineRgn(rgn, rgn, cut, RGN_DIFF);
            CombineRgn(rgn, rgn, t, RGN_OR);
            DeleteObject(cut);
            DeleteObject(t);
            SetWindowRgn(h, rgn, 1);
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

    pub fn screen_tag(x: i32, y: i32) -> String {
        use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
        unsafe {
            let m = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
            let mut mi: MONITORINFO = std::mem::zeroed();
            mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if !m.is_null() && GetMonitorInfoW(m, &mut mi) != 0 {
                let r = mi.rcMonitor;
                return format!("{},{},{}x{}", r.left, r.top, r.right - r.left, r.bottom - r.top);
            }
        }
        let (l, t, w, h) = primary_screen();
        format!("{l},{t},{w}x{h}")
    }

    pub fn monitor_of(hwnd: isize) -> (i32, i32, i32, i32) {
        use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
        unsafe {
            let m = MonitorFromWindow(hwnd as HWND, MONITOR_DEFAULTTONEAREST);
            let mut mi: MONITORINFO = std::mem::zeroed();
            mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if !m.is_null() && GetMonitorInfoW(m, &mut mi) != 0 {
                let r = mi.rcMonitor;
                return (r.left, r.top, r.right - r.left, r.bottom - r.top);
            }
        }
        virtual_screen()
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

    pub fn own_windows() -> Vec<(i32, i32, i32, i32)> {
        unsafe extern "system" fn each(h: HWND, out: LPARAM) -> windows_sys::core::BOOL {
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(h, &mut pid) };
            let clear = unsafe { GetWindowLongW(h, GWL_EXSTYLE) } as u32 & WS_EX_TRANSPARENT != 0;
            if pid == unsafe { GetCurrentProcessId() } && !clear && unsafe { IsWindowVisible(h) } != 0 && unsafe { IsIconic(h) } == 0 {
                let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                if unsafe { GetWindowRect(h, &mut r) } != 0 && r.right > r.left && r.bottom > r.top {
                    unsafe { (*(out as *mut Vec<(i32, i32, i32, i32)>)).push((r.left, r.top, r.right, r.bottom)) };
                }
            }
            1
        }
        let mut v: Vec<(i32, i32, i32, i32)> = vec![];
        unsafe {
            EnumWindows(Some(each), &mut v as *mut _ as LPARAM);
        }
        v
    }

    pub fn game_rect() -> Option<(i32, i32, i32, i32)> {
        unsafe extern "system" fn each(h: HWND, out: LPARAM) -> windows_sys::core::BOOL {
            let mut buf = [0u16; 128];
            let n = unsafe { GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32) };
            let title = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
            if title.starts_with("MapleStory Worlds") && unsafe { IsWindowVisible(h) } != 0 && unsafe { IsIconic(h) } == 0 {
                unsafe { *(out as *mut HWND) = h };
                return 0;
            }
            1
        }
        let mut h: HWND = std::ptr::null_mut();
        unsafe {
            EnumWindows(Some(each), &mut h as *mut HWND as LPARAM);
            if h.is_null() {
                return None;
            }
            let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetClientRect(h, &mut r);
            let mut p = POINT { x: 0, y: 0 };
            windows_sys::Win32::Graphics::Gdi::ClientToScreen(h, &mut p);
            let (w, hh) = (r.right - r.left, r.bottom - r.top);
            (w >= 320 && hh >= 180).then_some((p.x, p.y, w, hh))
        }
    }

    pub fn marks(m: Option<crate::papulatus::Marks>) {
        use std::sync::{mpsc, Mutex, OnceLock};
        static TX: OnceLock<Mutex<mpsc::Sender<Option<crate::papulatus::Marks>>>> = OnceLock::new();
        let tx = TX.get_or_init(|| {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || marks_loop(rx));
            Mutex::new(tx)
        });
        let _ = tx.lock().unwrap().send(m);
    }

    fn marks_loop(rx: std::sync::mpsc::Receiver<Option<crate::papulatus::Marks>>) {
        use std::sync::mpsc::RecvTimeoutError;
        use std::time::{Duration, Instant};
        use windows_sys::Win32::Graphics::Gdi::*;
        unsafe {
            let inst = windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null());
            let cls = wide("ArtaleMarks");
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(DefWindowProcW);
            wc.hInstance = inst;
            wc.lpszClassName = cls.as_ptr();
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                cls.as_ptr(),
                wide("").as_ptr(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                inst,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return;
            }
            let mem = CreateCompatibleDC(std::ptr::null_mut());
            let mut dib: (HBITMAP, *mut u32, i32, i32) = (std::ptr::null_mut(), std::ptr::null_mut(), 0, 0);
            let mut sprite: (f32, usize, usize, Vec<u32>) = (0.0, 0, 0, vec![]);
            let mut cur: Option<crate::papulatus::Marks> = None;
            let mut shown = false;
            let mut last: Option<(i32, i32)> = None;
            let mut top_at = Instant::now();
            let start = Instant::now();
            loop {
                let mut msg: MSG = std::mem::zeroed();
                while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                match rx.recv_timeout(Duration::from_millis(16)) {
                    Ok(m) => {
                        cur = m;
                        while let Ok(m) = rx.try_recv() {
                            cur = m;
                        }
                        last = None;
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
                let Some(m) = cur.as_ref().filter(|m| m.w > 0 && !m.xs.is_empty()) else {
                    if shown {
                        ShowWindow(hwnd, SW_HIDE);
                        shown = false;
                    }
                    continue;
                };
                if sprite.0 != m.u {
                    let (w, h, px) = crate::papulatus::arrow_sprite(m.u);
                    sprite = (m.u, w, h, px);
                }
                let (sw, sh) = (sprite.1 as i32, sprite.2 as i32);
                if sw == 0 {
                    continue;
                }
                let amp = (3.0 * m.u).round().max(1.0) as i32;
                let bob = (amp as f64 * (start.elapsed().as_secs_f64() * std::f64::consts::TAU / 1.2).sin()).round() as i32;
                let (w, h) = (m.w, sh + 2 * amp);
                let top = m.floor + (crate::papulatus::ARROW_TOP * m.u).round() as i32 - amp;
                if last == Some((bob, top)) {
                    continue;
                }
                last = Some((bob, top));
                if dib.2 != w || dib.3 != h {
                    if !dib.0.is_null() {
                        DeleteObject(dib.0 as _);
                    }
                    let mut bi: BITMAPINFO = std::mem::zeroed();
                    bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                    bi.bmiHeader.biWidth = w;
                    bi.bmiHeader.biHeight = -h;
                    bi.bmiHeader.biPlanes = 1;
                    bi.bmiHeader.biBitCount = 32;
                    bi.bmiHeader.biCompression = BI_RGB;
                    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
                    let b = CreateDIBSection(mem, &bi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
                    if b.is_null() {
                        dib = (std::ptr::null_mut(), std::ptr::null_mut(), 0, 0);
                        continue;
                    }
                    SelectObject(mem, b as _);
                    dib = (b, bits as *mut u32, w, h);
                }
                let buf = std::slice::from_raw_parts_mut(dib.1, (w * h) as usize);
                buf.fill(0);
                for &cx in &m.xs {
                    let x0 = cx - sw / 2;
                    let y0 = amp + bob;
                    for y in 0..sh {
                        let yy = y0 + y;
                        if yy < 0 || yy >= h {
                            continue;
                        }
                        for x in 0..sw {
                            let xx = x0 + x;
                            if xx >= 0 && xx < w {
                                buf[(yy * w + xx) as usize] = sprite.3[(y * sw + x) as usize];
                            }
                        }
                    }
                }
                let pos = POINT { x: m.left, y: top };
                let size = SIZE { cx: w, cy: h };
                let src = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
                UpdateLayeredWindow(hwnd, std::ptr::null_mut(), &pos, &size, mem, &src, 0, &blend, ULW_ALPHA);
                if !shown {
                    ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                    shown = true;
                }
                if top_at.elapsed() >= Duration::from_secs(1) {
                    top_at = Instant::now();
                    SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                }
            }
        }
    }
}

#[cfg(windows)]
pub use win::*;

#[cfg(target_os = "macos")]
#[path = "platform_mac.rs"]
mod mac;
#[cfg(target_os = "macos")]
pub use mac::*;

#[cfg(not(any(windows, target_os = "macos")))]
mod other {
    use super::*;
    pub struct InstanceGuard;
    pub const HOTKEY_NAMES: [&str; 2] = ["Ctrl+F10", "Ctrl+F12"];
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
    pub fn round_corners(_hwnd: isize, _w: u32, _h: u32, _radius: f32, _scale: f64) {}
    pub fn hint_window(_hwnd: isize, _frame: (i32, i32, i32, i32), _hole: (i32, i32, i32, i32), _tab: (i32, i32, i32, i32), _radius: i32) {}
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
    pub fn monitor_of(_hwnd: isize) -> (i32, i32, i32, i32) {
        virtual_screen()
    }
    pub fn screen_tag(_x: i32, _y: i32) -> String {
        "0,0,1920x1080".into()
    }
    pub fn virtual_screen() -> (i32, i32, i32, i32) {
        (0, 0, 1920, 1080)
    }
    pub fn primary_screen() -> (i32, i32, i32, i32) {
        (0, 0, 1920, 1080)
    }
    pub fn below_normal_priority() {}
    pub fn game_rect() -> Option<(i32, i32, i32, i32)> {
        None
    }
    pub fn own_windows() -> Vec<(i32, i32, i32, i32)> {
        vec![]
    }
    pub fn marks(_m: Option<crate::papulatus::Marks>) {}
}

#[cfg(not(any(windows, target_os = "macos")))]
pub use other::*;
