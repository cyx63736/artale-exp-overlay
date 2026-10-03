use crate::img::Bgr;
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGImage;
use objc2_foundation::{NSArray, NSError};
use objc2_screen_capture_kit::{SCContentFilter, SCDisplay, SCRunningApplication, SCScreenshotManager, SCShareableContent, SCStreamConfiguration};
use std::ffi::c_void;
use std::sync::mpsc::{channel, Sender};
use std::time::Duration;

pub const DENIED_MSG: &str = "還沒有「螢幕錄製」權限，讀不到畫面。\n請到「系統設定 → 隱私權與安全性 → 螢幕與系統錄音」打開「經驗收益計算器」，再把本程式關掉重開。";

#[allow(non_snake_case)]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
    fn CGMainDisplayID() -> u32;
    fn CGDisplayBounds(display: u32) -> CGRect;
    fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
    fn CGDisplayCopyDisplayMode(display: u32) -> *mut c_void;
    fn CGDisplayModeGetPixelWidth(mode: *mut c_void) -> usize;
    fn CGDisplayModeRelease(mode: *mut c_void);
    fn CGImageGetWidth(img: *const c_void) -> usize;
    fn CGImageGetHeight(img: *const c_void) -> usize;
    fn CGImageGetColorSpace(img: *const c_void) -> *mut c_void;
    fn CGColorSpaceCreateDeviceRGB() -> *mut c_void;
    fn CGColorSpaceRelease(cs: *mut c_void);
    fn CGBitmapContextCreate(data: *mut c_void, w: usize, h: usize, bpc: usize, bpr: usize, cs: *mut c_void, info: u32) -> *mut c_void;
    fn CGContextDrawImage(ctx: *mut c_void, rect: CGRect, img: *const c_void);
    fn CGContextRelease(ctx: *mut c_void);
}

pub fn has_permission() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

pub fn request_permission() -> bool {
    unsafe { CGRequestScreenCaptureAccess() }
}

pub fn scale() -> f64 {
    scale_of(unsafe { CGMainDisplayID() })
}

pub fn scale_of(d: u32) -> f64 {
    unsafe {
        let pts = CGDisplayBounds(d).size.width;
        let mode = CGDisplayCopyDisplayMode(d);
        if mode.is_null() || pts <= 0.0 {
            return 1.0;
        }
        let px = CGDisplayModeGetPixelWidth(mode) as f64;
        CGDisplayModeRelease(mode);
        if px > 0.0 { px / pts } else { 1.0 }
    }
}

pub fn screens() -> Vec<(u32, CGRect)> {
    let mut ids = [0u32; 16];
    let mut n = 0u32;
    unsafe {
        if CGGetActiveDisplayList(16, ids.as_mut_ptr(), &mut n) != 0 {
            let d = CGMainDisplayID();
            return vec![(d, CGDisplayBounds(d))];
        }
        ids[..n as usize].iter().map(|&d| (d, CGDisplayBounds(d))).collect()
    }
}

pub fn display_sig() -> String {
    screens().iter().map(|(d, b)| format!("{},{},{}x{}@{}", b.origin.x, b.origin.y, b.size.width, b.size.height, scale_of(*d))).collect::<Vec<_>>().join(" ")
}

pub fn main_screen_px() -> (i32, i32, i32, i32) {
    let s = scale();
    let b = unsafe { CGDisplayBounds(CGMainDisplayID()) };
    (0, 0, (b.size.width * s).round() as i32, (b.size.height * s).round() as i32)
}

struct SendBox<T>(T);
unsafe impl<T> Send for SendBox<T> {}

fn wait<T>(f: impl FnOnce(Sender<SendBox<Option<T>>>)) -> Option<T> {
    let (tx, rx) = channel();
    f(tx);
    rx.recv_timeout(Duration::from_secs(3)).ok().and_then(|b| b.0)
}

fn shareable() -> Option<Retained<SCShareableContent>> {
    wait(|tx| {
        let block = RcBlock::new(move |content: *mut SCShareableContent, _err: *mut NSError| {
            let c = unsafe { Retained::retain(content) };
            let _ = tx.send(SendBox(c));
        });
        unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&block) };
    })
}

struct Screen {
    id: u32,
    bounds: CGRect,
    filter: Retained<SCContentFilter>,
}

pub struct Grabber {
    screens: Vec<Screen>,
    scale: f64,
    sig: String,
}

pub fn grab_once(x: i32, y: i32, w: i32, h: i32) -> Option<Bgr> {
    std::thread::spawn(move || Grabber::new().and_then(|g| g.grab(x, y, w, h))).join().ok().flatten()
}

impl Grabber {
    pub fn new() -> Option<Self> {
        objc2::rc::autoreleasepool(|_| Self::make())
    }

    fn make() -> Option<Self> {
        if !has_permission() {
            return None;
        }
        let content = shareable()?;
        let me = std::process::id() as i32;
        let apps: Vec<Retained<SCRunningApplication>> = unsafe { content.applications() }.iter().filter(|a| unsafe { a.processID() } == me).collect();
        let apps = NSArray::from_retained_slice(&apps);
        let none = NSArray::new();
        let displays: Vec<Retained<SCDisplay>> = unsafe { content.displays() }.iter().collect();
        let screens: Vec<Screen> = screens()
            .into_iter()
            .filter_map(|(id, bounds)| {
                let d = displays.iter().find(|d| unsafe { d.displayID() } == id)?;
                let filter = unsafe { SCContentFilter::initWithDisplay_excludingApplications_exceptingWindows(SCContentFilter::alloc(), d, &apps, &none) };
                Some(Screen { id, bounds, filter })
            })
            .collect();
        if screens.is_empty() {
            return None;
        }
        Some(Grabber { screens, scale: scale(), sig: display_sig() })
    }

    pub fn grab(&self, x: i32, y: i32, w: i32, h: i32) -> Option<Bgr> {
        objc2::rc::autoreleasepool(|_| self.shot(x, y, w, h))
    }

    pub fn stale(&self) -> bool {
        self.sig != display_sig()
    }

    fn shot(&self, x: i32, y: i32, w: i32, h: i32) -> Option<Bgr> {
        if w <= 0 || h <= 0 {
            return None;
        }
        let s = self.scale;
        let (px, py, pw, ph) = (x as f64 / s, y as f64 / s, w as f64 / s, h as f64 / s);
        let (cx, cy) = (px + pw / 2.0, py + ph / 2.0);
        let inside = |b: &CGRect| cx >= b.origin.x && cx < b.origin.x + b.size.width && cy >= b.origin.y && cy < b.origin.y + b.size.height;
        let main = unsafe { CGMainDisplayID() };
        let scr = self.screens.iter().find(|sc| inside(&sc.bounds)).or_else(|| self.screens.iter().find(|sc| sc.id == main)).unwrap_or(&self.screens[0]);
        let rect = CGRect::new(CGPoint::new(px - scr.bounds.origin.x, py - scr.bounds.origin.y), CGSize::new(pw, ph));
        let cfg = unsafe { SCStreamConfiguration::new() };
        unsafe {
            cfg.setSourceRect(rect);
            cfg.setWidth(w as usize);
            cfg.setHeight(h as usize);
            cfg.setShowsCursor(false);
        }
        let filter = scr.filter.clone();
        let out = wait(|tx| {
            let block = RcBlock::new(move |img: *mut CGImage, _err: *mut NSError| {
                let _ = tx.send(SendBox(if img.is_null() { None } else { Some(to_bgr(img as *const c_void)) }));
            });
            unsafe { SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(&filter, &cfg, Some(&block)) };
        })?;
        let mut img = out?;
        if img.w != w as usize || img.h != h as usize {
            img = fit(&img, w as usize, h as usize);
        }
        Some(img)
    }
}

fn to_bgr(img: *const c_void) -> Option<Bgr> {
    unsafe {
        let (w, h) = (CGImageGetWidth(img), CGImageGetHeight(img));
        if w == 0 || h == 0 {
            return None;
        }
        let mut buf = vec![0u8; w * h * 4];
        let own = CGImageGetColorSpace(img);
        let info = 6 | (2 << 12);
        let mut ctx = CGBitmapContextCreate(buf.as_mut_ptr() as _, w, h, 8, w * 4, own, info);
        let mut rgb = std::ptr::null_mut();
        if ctx.is_null() {
            rgb = CGColorSpaceCreateDeviceRGB();
            ctx = CGBitmapContextCreate(buf.as_mut_ptr() as _, w, h, 8, w * 4, rgb, info);
        }
        if ctx.is_null() {
            if !rgb.is_null() {
                CGColorSpaceRelease(rgb);
            }
            return None;
        }
        CGContextDrawImage(ctx, CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(w as f64, h as f64)), img);
        CGContextRelease(ctx);
        if !rgb.is_null() {
            CGColorSpaceRelease(rgb);
        }
        let mut out = Bgr::new(w, h);
        for (d, s) in out.data.chunks_exact_mut(3).zip(buf.chunks_exact(4)) {
            d.copy_from_slice(&s[..3]);
        }
        Some(out)
    }
}

fn fit(img: &Bgr, w: usize, h: usize) -> Bgr {
    let mut out = Bgr::new(w, h);
    let (cw, ch) = (w.min(img.w), h.min(img.h));
    for y in 0..ch {
        out.data[y * w * 3..(y * w + cw) * 3].copy_from_slice(&img.data[y * img.w * 3..(y * img.w + cw) * 3]);
    }
    out
}
