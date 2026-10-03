use super::*;
use objc2::rc::Retained;
use objc2::{define_class, AnyThread, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAlert, NSAlertStyle, NSApplication, NSApplicationActivationOptions, NSColor, NSEvent, NSPanel, NSRunningApplication, NSTrackingArea, NSTrackingAreaOptions, NSView,
    NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask, NSWorkspace,
};
use objc2_foundation::{NSPoint, NSString};
use std::sync::Mutex;

const FLOATING: isize = 3;
const STATUS: isize = 25;
const COVER: isize = 1000;

define_class!(
    #[unsafe(super(NSPanel, NSWindow, objc2_app_kit::NSResponder, objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ExpOverlayKeyPanel"]
    struct KeyPanel;

    impl KeyPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool {
            true
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main(&self) -> bool {
            false
        }
    }
);

static ORIG_CLASS: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());

pub fn restore_class(hwnd: isize) {
    let Some(w) = window(hwnd) else { return };
    let key = Retained::as_ptr(&w) as usize;
    let mut v = ORIG_CLASS.lock().unwrap();
    if let Some(i) = v.iter().position(|(k, _)| *k == key) {
        let (_, cls) = v.remove(i);
        unsafe extern "C" {
            fn object_setClass(obj: *mut std::ffi::c_void, cls: *const std::ffi::c_void) -> *const std::ffi::c_void;
        }
        unsafe { object_setClass(key as *mut _, cls as *const _) };
    }
    LEVELS.lock().unwrap().retain(|(h, _)| *h != hwnd);
}

fn into_panel<'a>(w: &'a NSWindow, cls: &objc2::runtime::AnyClass) -> &'a NSPanel {
    unsafe extern "C" {
        fn object_setClass(obj: *mut std::ffi::c_void, cls: *const std::ffi::c_void) -> *const std::ffi::c_void;
    }
    let orig = unsafe { object_setClass(w as *const NSWindow as *mut _, cls as *const _ as *const _) };
    ORIG_CLASS.lock().unwrap().push((w as *const NSWindow as usize, orig as usize));
    w.setStyleMask(w.styleMask() | NSWindowStyleMask::NonactivatingPanel);
    let panel = unsafe { &*(w as *const NSWindow as *const NSPanel) };
    panel.setWorksWhenModal(true);
    w.setHidesOnDeactivate(false);
    panel
}

pub fn key_panel(hwnd: isize) {
    if let Some(w) = window(hwnd) {
        into_panel(&w, KeyPanel::class());
    }
}

static PREV_APP: Mutex<Option<i32>> = Mutex::new(None);

static LEVELS: Mutex<Vec<(isize, isize)>> = Mutex::new(Vec::new());

fn set_level_of(hwnd: isize, level: isize) {
    let mut v = LEVELS.lock().unwrap();
    v.retain(|(h, _)| *h != hwnd);
    v.push((hwnd, level));
}

fn level_of(hwnd: isize) -> isize {
    LEVELS.lock().unwrap().iter().find(|(h, _)| *h == hwnd).map_or(FLOATING, |(_, l)| *l)
}

fn window(hwnd: isize) -> Option<Retained<NSWindow>> {
    if hwnd == 0 || MainThreadMarker::new().is_none() {
        return None;
    }
    let view = unsafe { &*(hwnd as *const NSView) };
    view.window()
}

fn scale() -> f64 {
    ocr::capture::scale()
}

fn main_h() -> f64 {
    let (_, _, _, h) = ocr::capture::main_screen_px();
    h as f64 / scale()
}

pub struct InstanceGuard;

pub fn single_instance() -> Instance {
    Instance::Only(InstanceGuard)
}

pub fn watch_quit(_f: impl Fn() + Send + 'static) {}

pub const HOTKEY_NAMES: [&str; 2] = ["⌃⌘1", "⌃⌘3"];

type HotkeySink = (Sender<Hotkey>, Box<dyn Fn() + Send>);
static HOTKEY_SINK: Mutex<Option<HotkeySink>> = Mutex::new(None);

#[repr(C)]
struct EventHotKeyID {
    signature: u32,
    id: u32,
}

#[repr(C)]
struct EventTypeSpec {
    class: u32,
    kind: u32,
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetApplicationEventTarget() -> *mut std::ffi::c_void;
    fn RegisterEventHotKey(code: u32, mods: u32, id: EventHotKeyID, target: *mut std::ffi::c_void, options: u32, out: *mut *mut std::ffi::c_void) -> i32;
    fn InstallEventHandler(
        target: *mut std::ffi::c_void,
        handler: extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, *mut std::ffi::c_void) -> i32,
        num: u32,
        list: *const EventTypeSpec,
        user: *mut std::ffi::c_void,
        out: *mut *mut std::ffi::c_void,
    ) -> i32;
    fn GetEventParameter(
        event: *mut std::ffi::c_void,
        name: u32,
        kind: u32,
        out_kind: *mut u32,
        size: usize,
        out_size: *mut usize,
        data: *mut std::ffi::c_void,
    ) -> i32;
}

const HOTKEY_SIG: u32 = u32::from_be_bytes(*b"ExpO");

extern "C" fn on_hotkey(_next: *mut std::ffi::c_void, event: *mut std::ffi::c_void, _user: *mut std::ffi::c_void) -> i32 {
    let mut id = EventHotKeyID { signature: 0, id: 0 };
    let ok = unsafe {
        GetEventParameter(
            event,
            u32::from_be_bytes(*b"----"),
            u32::from_be_bytes(*b"hkid"),
            std::ptr::null_mut(),
            std::mem::size_of::<EventHotKeyID>(),
            std::ptr::null_mut(),
            &mut id as *mut _ as *mut _,
        )
    };
    if ok != 0 || id.signature != HOTKEY_SIG {
        return -9874;
    }
    let key = if id.id == 1 { Hotkey::Pause } else { Hotkey::Toggle };
    if let Some((tx, wake)) = HOTKEY_SINK.lock().unwrap().as_ref() {
        let _ = tx.send(key);
        wake();
    }
    0
}

pub fn hotkeys(tx: Sender<Hotkey>, failed: Sender<Vec<&'static str>>, wake: impl Fn() + Send + 'static) {
    *HOTKEY_SINK.lock().unwrap() = Some((tx, Box::new(wake)));
    let target = unsafe { GetApplicationEventTarget() };
    let spec = EventTypeSpec { class: u32::from_be_bytes(*b"keyb"), kind: 5 };
    unsafe { InstallEventHandler(target, on_hotkey, 1, &spec, std::ptr::null_mut(), std::ptr::null_mut()) };
    const CONTROL_CMD: u32 = 4096 | 256;
    let keys = [(1, 0x12), (3, 0x14)];
    let mut bad = vec![];
    for (i, (id, code)) in keys.into_iter().enumerate() {
        let mut out = std::ptr::null_mut();
        if unsafe { RegisterEventHotKey(code, CONTROL_CMD, EventHotKeyID { signature: HOTKEY_SIG, id }, target, 0, &mut out) } != 0 {
            bad.push(HOTKEY_NAMES[i]);
        }
    }
    if !bad.is_empty() {
        let _ = failed.send(bad);
        if let Some((_, wake)) = HOTKEY_SINK.lock().unwrap().as_ref() {
            wake();
        }
    }
}

pub fn message_box(text: &str, flags: u32) -> i32 {
    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("{text}");
        return 0;
    };
    let app = NSApplication::sharedApplication(mtm);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str("經驗收益計算器"));
    alert.setInformativeText(&NSString::from_str(text));
    alert.setAlertStyle(match flags & 0xF0 {
        0x10 => NSAlertStyle::Critical,
        0x30 => NSAlertStyle::Warning,
        _ => NSAlertStyle::Informational,
    });
    let yes_no = flags & 0xF == 4;
    if yes_no {
        alert.addButtonWithTitle(&NSString::from_str("是"));
        alert.addButtonWithTitle(&NSString::from_str("否"));
    } else {
        alert.addButtonWithTitle(&NSString::from_str("好"));
    }
    let r = alert.runModal();
    match (yes_no, r) {
        (true, 1000) => 6,
        (true, _) => 7,
        _ => 1,
    }
}

fn all_spaces() -> NSWindowCollectionBehavior {
    NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::FullScreenAuxiliary | NSWindowCollectionBehavior::Stationary | NSWindowCollectionBehavior::IgnoresCycle
}

pub fn no_activate(hwnd: isize) {
    set_level_of(hwnd, STATUS);
    let Some(w) = window(hwnd) else { return };
    let panel = into_panel(&w, NSPanel::class());
    panel.setFloatingPanel(true);
    panel.setBecomesKeyOnlyIfNeeded(true);
    w.setCollectionBehavior(all_spaces());
    w.setLevel(STATUS);
    w.setOpaque(false);
    w.setBackgroundColor(Some(&NSColor::clearColor()));
    w.setHasShadow(true);
    if let Some(v) = w.contentView() {
        v.setWantsLayer(true);
        unsafe {
            let layer: *mut objc2::runtime::AnyObject = objc2::msg_send![&*v, layer];
            if !layer.is_null() {
                let _: () = objc2::msg_send![layer, setCornerRadius: crate::ui::OVERLAY_RADIUS as f64];
                let _: () = objc2::msg_send![layer, setMasksToBounds: true];
            }
        }
    }
    let view = unsafe { &*(hwnd as *const NSView) };
    let opts = NSTrackingAreaOptions::MouseMoved | NSTrackingAreaOptions::MouseEnteredAndExited | NSTrackingAreaOptions::ActiveAlways | NSTrackingAreaOptions::InVisibleRect;
    let area = unsafe { NSTrackingArea::initWithRect_options_owner_userInfo(NSTrackingArea::alloc(), view.bounds(), opts, Some(view), None) };
    view.addTrackingArea(&area);
}

pub fn hint_window(hwnd: isize, frame: (i32, i32, i32, i32), hole: (i32, i32, i32, i32), tab: (i32, i32, i32, i32), radius: i32) {
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_core_graphics::{CGColor, CGMutablePath};
    use objc2_quartz_core::{kCAFillRuleEvenOdd, CALayer, CAShapeLayer};
    set_level_of(hwnd, STATUS);
    let Some(w) = window(hwnd) else { return };
    let panel = into_panel(&w, NSPanel::class());
    panel.setFloatingPanel(true);
    panel.setBecomesKeyOnlyIfNeeded(true);
    w.setCollectionBehavior(all_spaces());
    w.setLevel(STATUS);
    w.setIgnoresMouseEvents(true);
    w.setOpaque(false);
    w.setBackgroundColor(Some(&NSColor::clearColor()));
    w.setHasShadow(false);
    let Some(v) = w.contentView() else { return };
    v.setWantsLayer(true);
    let layer: Option<Retained<CALayer>> = unsafe { objc2::msg_send![&*v, layer] };
    let Some(layer) = layer else { return };
    let s = scale();
    let h = layer.bounds().size.height;
    let down = layer.contentsAreFlipped();
    let path = CGMutablePath::new();
    let add = |r: (i32, i32, i32, i32), rad: i32| {
        let (x, y, rw, rh) = (r.0 as f64 / s, r.1 as f64 / s, r.2 as f64 / s, r.3 as f64 / s);
        let y = if down { y } else { h - y - rh };
        let rad = (rad.max(0) as f64 / s).min(rw / 2.0).min(rh / 2.0);
        let rect = CGRect::new(CGPoint::new(x, y), CGSize::new(rw, rh));
        unsafe { CGMutablePath::add_rounded_rect(Some(&path), std::ptr::null(), rect, rad, rad) };
    };
    let bw = hole.0 - frame.0;
    add(frame, radius);
    add(hole, (radius - bw).max(0));
    add(tab, radius);
    let mask = CAShapeLayer::new();
    mask.setFrame(layer.bounds());
    mask.setPath(Some(&path));
    mask.setFillRule(unsafe { kCAFillRuleEvenOdd });
    mask.setFillColor(Some(&CGColor::new_generic_rgb(0.0, 0.0, 0.0, 1.0)));
    unsafe { layer.setMask(Some(&mask)) };
}

pub fn round_corners(hwnd: isize, _w: u32, _h: u32, radius: f32, scale: f64) {
    let Some(w) = window(hwnd) else { return };
    let Some(v) = w.contentView() else { return };
    let r = radius as f64 * scale / w.backingScaleFactor();
    unsafe {
        let layer: *mut objc2::runtime::AnyObject = objc2::msg_send![&*v, layer];
        if !layer.is_null() {
            let _: () = objc2::msg_send![layer, setCornerRadius: r];
        }
    }
}

pub fn cover_screen(hwnd: isize) {
    set_level_of(hwnd, COVER);
    key_panel(hwnd);
    if let Some(w) = window(hwnd) {
        w.setCollectionBehavior(all_spaces());
        w.setLevel(COVER);
    }
}

pub fn set_alpha(hwnd: isize, alpha: f32) {
    if let Some(w) = window(hwnd) {
        let a = alpha.clamp(0.1, 1.0) as f64;
        if (w.alphaValue() - a).abs() > 0.001 {
            w.setAlphaValue(a);
        }
    }
}

pub fn dpi_aware() {}

pub fn dll_dir(_dir: &std::path::Path) {}

pub fn keep_top(hwnd: isize) {
    if let Some(w) = window(hwnd) {
        let level = level_of(hwnd);
        if level == FLOATING {
            w.setCollectionBehavior(NSWindowCollectionBehavior::MoveToActiveSpace | NSWindowCollectionBehavior::FullScreenAuxiliary);
        }
        w.setLevel(level);
        if w.isVisible() {
            w.orderFrontRegardless();
        }
    }
}

pub fn show_quiet(hwnd: isize, show: bool) {
    if let Some(w) = window(hwnd) {
        if show {
            w.orderFrontRegardless();
        } else {
            w.orderOut(None);
        }
    }
}

pub fn move_quiet(hwnd: isize, x: i32, y: i32) {
    if let Some(w) = window(hwnd) {
        let s = scale();
        w.setFrameTopLeftPoint(NSPoint::new(x as f64 / s, main_h() - y as f64 / s));
    }
}

pub fn cursor_pos() -> (i32, i32) {
    let p = NSEvent::mouseLocation();
    let s = scale();
    ((p.x * s).round() as i32, ((main_h() - p.y) * s).round() as i32)
}

pub fn window_pos(hwnd: isize) -> (i32, i32) {
    match window(hwnd) {
        Some(w) => {
            let f = w.frame();
            let s = scale();
            ((f.origin.x * s).round() as i32, ((main_h() - f.origin.y - f.size.height) * s).round() as i32)
        }
        None => (0, 0),
    }
}

pub fn find_window(_title: &str) -> Option<isize> {
    None
}

pub fn ime_off(_hwnd: isize) {}

pub fn force_focus(hwnd: isize) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let _ = mtm;
    let front = foreground() as i32;
    if front > 0 && front != std::process::id() as i32 {
        *PREV_APP.lock().unwrap() = Some(front);
    }
    if let Some(w) = window(hwnd) {
        w.makeKeyAndOrderFront(None);
    }
}

pub fn foreground() -> isize {
    NSWorkspace::sharedWorkspace().frontmostApplication().map_or(0, |a| a.processIdentifier() as isize)
}

pub fn set_foreground(pid: isize) {
    if pid <= 0 || pid == std::process::id() as isize {
        return;
    }
    if let Some(a) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid as i32) {
        a.activateWithOptions(NSApplicationActivationOptions::empty());
    }
}

pub fn restore_focus() {
    if let Some(pid) = PREV_APP.lock().unwrap().take() {
        if foreground() == std::process::id() as isize {
            set_foreground(pid as isize);
        }
    }
}

pub fn on_screen(x: i32, y: i32) -> bool {
    let s = scale();
    let (px, py) = (x as f64 / s, y as f64 / s);
    ocr::capture::screens().iter().any(|(_, b)| px >= b.origin.x && px < b.origin.x + b.size.width && py >= b.origin.y && py < b.origin.y + b.size.height)
}

pub fn monitor_of(hwnd: isize) -> (i32, i32, i32, i32) {
    let Some(w) = window(hwnd) else { return primary_screen() };
    let f = w.frame();
    let (cx, cy) = (f.origin.x + f.size.width / 2.0, main_h() - f.origin.y - f.size.height / 2.0);
    let s = scale();
    ocr::capture::screens()
        .iter()
        .map(|(_, b)| b)
        .find(|b| cx >= b.origin.x && cx < b.origin.x + b.size.width && cy >= b.origin.y && cy < b.origin.y + b.size.height)
        .map_or_else(primary_screen, |b| {
            ((b.origin.x * s).round() as i32, (b.origin.y * s).round() as i32, (b.size.width * s).round() as i32, (b.size.height * s).round() as i32)
        })
}

pub fn screen_tag(x: i32, y: i32) -> String {
    let s = scale();
    let (px, py) = (x as f64 / s, y as f64 / s);
    let all = ocr::capture::screens();
    let main = all.iter().find(|(_, b)| b.origin.x == 0.0 && b.origin.y == 0.0).or(all.first());
    let hit = all.iter().find(|(_, b)| px >= b.origin.x && px < b.origin.x + b.size.width && py >= b.origin.y && py < b.origin.y + b.size.height);
    match hit.or(main) {
        Some((d, b)) => format!("{},{},{}x{}@{}", b.origin.x, b.origin.y, b.size.width, b.size.height, ocr::capture::scale_of(*d)),
        None => "0,0,0x0@1".into(),
    }
}

pub fn virtual_screen() -> (i32, i32, i32, i32) {
    ocr::capture::main_screen_px()
}

pub fn primary_screen() -> (i32, i32, i32, i32) {
    ocr::capture::main_screen_px()
}

pub fn boost(on: bool) {
    unsafe extern "C" {
        fn pthread_set_qos_class_self_np(qos: u32, rel: i32) -> i32;
    }
    unsafe {
        pthread_set_qos_class_self_np(if on { 0x19 } else { 0x11 }, 0);
    }
}

pub fn below_normal_priority() {
    unsafe extern "C" {
        fn pthread_set_qos_class_self_np(qos: u32, rel: i32) -> i32;
    }
    unsafe {
        pthread_set_qos_class_self_np(0x11, 0);
    }
}
