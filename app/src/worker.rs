use crate::files::Paths;
use crate::platform;
use serde_json::{Map, Value};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use tracker::OcrOut;

pub enum Msg {
    Ready,
    Fatal(String),
    Err(String),
    Ocr(OcrOut),
    Logs(Vec<String>),
}

pub struct Shared {
    pub cfg: Mutex<Map<String, Value>>,
    wake: Mutex<bool>,
    cv: Condvar,
}

impl Shared {
    pub fn new(cfg: Map<String, Value>) -> Arc<Shared> {
        Arc::new(Shared { cfg: Mutex::new(cfg), wake: Mutex::new(false), cv: Condvar::new() })
    }
    pub fn wake(&self) {
        *self.wake.lock().unwrap() = true;
        self.cv.notify_all();
    }
    fn wait(&self, d: Duration) {
        let g = self.wake.lock().unwrap();
        let (mut g, _) = self.cv.wait_timeout_while(g, d, |w| !*w).unwrap();
        *g = false;
    }
}

#[cfg(windows)]
struct GdiScreen(Option<ocr::capture::Grabber>, u32);

#[cfg(windows)]
impl reader::Screen for GdiScreen {
    fn grab(&mut self, x: i32, y: i32, w: i32, h: i32) -> Option<ocr::img::Bgr> {
        if self.0.is_none() {
            self.0 = ocr::capture::Grabber::new();
        }
        let r = self.0.as_ref()?.grab(x, y, w, h);
        if r.is_none() {
            self.1 += 1;
            if self.1 >= 3 {
                self.1 = 0;
                self.0 = None;
            }
        } else {
            self.1 = 0;
        }
        r
    }
}

pub fn spawn(paths: Paths, shared: Arc<Shared>, tx: Sender<Msg>, repaint: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        let send = |m: Msg| {
            let _ = tx.send(m);
            repaint();
        };
        crate::platform::dll_dir(&paths.models);
        let engine = match ocr::Ocr::new(&paths.models.join("onnxruntime.dll"), &paths.models) {
            Ok(e) => e,
            Err(e) => {
                send(Msg::Fatal(format!("OCR 無法載入：{e}")));
                return;
            }
        };
        platform::below_normal_priority();
        let mut rd = reader::Reader::new(engine);
        rd.fonts_path = Some(paths.digits.clone());
        rd.tpl_paths = [Some(paths.tpl[0].clone()), Some(paths.tpl[1].clone())];
        rd.load_fonts();
        #[cfg(windows)]
        let mut screen = GdiScreen(None, 0);
        send(Msg::Ready);
        let mut fast = 0u32;
        loop {
            let cfg = shared.cfg.lock().unwrap().clone();
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                #[cfg(windows)]
                {
                    rd.tick(&cfg, &mut screen)
                }
                #[cfg(not(windows))]
                {
                    (None, false)
                }
            }));
            let bag = match r {
                Ok((out, bag)) => {
                    if !rd.logs.is_empty() {
                        send(Msg::Logs(std::mem::take(&mut rd.logs)));
                    }
                    if let Some(o) = out {
                        send(Msg::Ocr(o));
                    }
                    bag
                }
                Err(e) => {
                    let s = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                    send(Msg::Err(format!("自動讀取出錯：{s}")));
                    false
                }
            };
            fast = if bag { fast + 1 } else { 0 };
            let ocr_sec = cfg.get("ocr_sec").and_then(|v| v.as_f64()).unwrap_or(3.0);
            let delay = if fast > 0 && fast <= 12 { 0.4 } else { ocr_sec.max(1.0) };
            shared.wait(Duration::from_secs_f64(delay));
        }
    });
}
