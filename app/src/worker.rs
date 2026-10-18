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
    Auto(Box<reader::auto::Found>, Box<ocr::img::Bgr>),
    Boss(crate::boss::Out),
    Zakum(Option<crate::zakum::Line>),
    Papu(Option<crate::papulatus::Water>),
    PapuMob(Option<crate::papulatus::Mob>),
    Update(crate::update::Event),
}

pub struct Shared {
    pub cfg: Mutex<Map<String, Value>>,
    pub auto: Mutex<Option<ocr::img::Bgr>>,
    wake: Mutex<bool>,
    cv: Condvar,
}

impl Shared {
    pub fn new(cfg: Map<String, Value>) -> Arc<Shared> {
        Arc::new(Shared { cfg: Mutex::new(cfg), auto: Mutex::new(None), wake: Mutex::new(false), cv: Condvar::new() })
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

#[cfg(any(windows, target_os = "macos"))]
struct GdiScreen(Option<ocr::capture::Grabber>, u32);

#[cfg(any(windows, target_os = "macos"))]
impl reader::Screen for GdiScreen {
    fn grab(&mut self, x: i32, y: i32, w: i32, h: i32) -> Option<ocr::img::Bgr> {
        #[cfg(target_os = "macos")]
        if self.0.as_ref().map_or(false, |g| g.stale()) {
            self.0 = None;
        }
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
    let repaint: Arc<Mutex<Box<dyn Fn() + Send>>> = Arc::new(Mutex::new(Box::new(repaint)));
    let gate: Arc<Mutex<Option<(std::time::Instant, bool, u32)>>> = Arc::new(Mutex::new(None));
    #[cfg(any(windows, target_os = "macos"))]
    spawn_zakum(paths.clone(), shared.clone(), gate.clone(), tx.clone(), repaint.clone());
    let papu: Arc<Mutex<Option<(std::time::Instant, u32)>>> = Arc::new(Mutex::new(None));
    #[cfg(any(windows, target_os = "macos"))]
    spawn_papu(paths.clone(), shared.clone(), papu.clone(), tx.clone(), repaint.clone());
    std::thread::spawn(move || {
        let send = |m: Msg| {
            let _ = tx.send(m);
            (repaint.lock().unwrap())();
        };
        crate::platform::dll_dir(&paths.models);
        let engine = match ocr::Ocr::new(&paths.models.join(if cfg!(target_os = "macos") { "libonnxruntime.dylib" } else { "onnxruntime.dll" }), &paths.models) {
            Ok(e) => e,
            Err(e) => {
                send(Msg::Fatal(format!("OCR 無法載入：{e}")));
                return;
            }
        };
        platform::below_normal_priority();
        let mut rd = reader::Reader::new(engine);
        rd.fonts_path = Some(paths.digits.clone());
        rd.tpl_paths = std::array::from_fn(|k| Some(paths.tpl[k].clone()));
        rd.load_fonts();
        #[cfg(target_os = "macos")]
        if !ocr::capture::has_permission() {
            ocr::capture::request_permission();
            send(Msg::Fatal(ocr::capture::DENIED_MSG.into()));
            return;
        }
        #[cfg(any(windows, target_os = "macos"))]
        let mut screen = GdiScreen(None, 0);
        send(Msg::Ready);
        let mut fast = 0u32;
        let mut boss = crate::boss::Watcher::default();
        let mut not_papu = 0u32;
        loop {
            let pending = shared.auto.lock().unwrap().take();
            if let Some(img) = pending {
                #[cfg(target_os = "macos")]
                let down = if ocr::capture::scale() >= 2.0 && img.w >= 2800 { 2 } else { 1 };
                #[cfg(not(target_os = "macos"))]
                let down = 1;
                #[cfg(target_os = "macos")]
                platform::boost(true);
                let mut tmp = ocr::Ocr::new(&paths.models.join(if cfg!(target_os = "macos") { "libonnxruntime.dylib" } else { "onnxruntime.dll" }), &paths.models).ok();
                let engine = tmp.as_mut().unwrap_or(&mut rd.engine);
                let mut find = |down: usize| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| reader::auto::find_with(&img, &mut *engine, down))).unwrap_or_default();
                let mut f = find(down);
                if down > 1 && (f.exp.is_none() || f.inv.is_none() || f.meso.is_none() || f.pairs.is_empty()) {
                    f = find(1);
                }
                drop(tmp);
                #[cfg(target_os = "macos")]
                platform::boost(false);
                let png = ::image::RgbImage::from_fn(img.w as u32, img.h as u32, |x, y| {
                    let i = (y as usize * img.w + x as usize) * 3;
                    ::image::Rgb([img.data[i + 2], img.data[i + 1], img.data[i]])
                });
                send(Msg::Auto(Box::new(f), Box::new(img)));
                let _ = png.save(std::env::temp_dir().join("artale_autoframe_last.png"));
            }
            let cfg = shared.cfg.lock().unwrap().clone();
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                #[cfg(any(windows, target_os = "macos"))]
                {
                    rd.tick(&cfg, &mut screen)
                }
                #[cfg(not(any(windows, target_os = "macos")))]
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
            #[cfg(any(windows, target_os = "macos"))]
            if crate::boss::enabled(&cfg) {
                if let Ok(Some(m)) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| boss.tick(&cfg, &mut rd.engine, &mut screen))) {
                    if let crate::boss::Out::Found(_, r) | crate::boss::Out::Read(Some(r)) = &m {
                        let prev = gate.lock().unwrap().map_or(false, |g| g.1);
                        if let Some(p3) = crate::zakum::zakum_phase(&r.name, r.hp, r.pct, prev) {
                            *gate.lock().unwrap() = Some((std::time::Instant::now(), p3, r.pct));
                        }
                        let was = papu.lock().unwrap().filter(|g| g.0.elapsed().as_secs_f64() < 20.0).map(|g| g.1);
                        match crate::papulatus::papu_pct(&r.name, r.hp, r.pct, was.is_some()).filter(|&p| p > 0 || was.map_or(false, |q| q <= 5)) {
                            Some(p) => {
                                not_papu = 0;
                                *papu.lock().unwrap() = Some((std::time::Instant::now(), p));
                            }
                            None => {
                                not_papu = not_papu.saturating_add(1);
                                if not_papu >= 2 {
                                    *papu.lock().unwrap() = None;
                                }
                            }
                        }
                    }
                    send(Msg::Boss(m));
                }
            }
            fast = if bag { fast + 1 } else { 0 };
            let ocr_sec = cfg.get("ocr_sec").and_then(|v| v.as_f64()).unwrap_or(3.0);
            let delay = if fast > 0 && fast <= 12 { 0.4 } else { ocr_sec.max(1.0) };
            shared.wait(Duration::from_secs_f64(delay));
        }
    });
}

#[cfg(any(windows, target_os = "macos"))]
fn spawn_zakum(paths: Paths, shared: Arc<Shared>, gate: Arc<Mutex<Option<(std::time::Instant, bool, u32)>>>, tx: Sender<Msg>, repaint: Arc<Mutex<Box<dyn Fn() + Send>>>) {
    std::thread::spawn(move || {
        use reader::Screen;
        platform::below_normal_priority();
        let mut zak = crate::zakum::Zakum::new(Some(paths.here.join("黑水偵測.log")));
        let mut screen = GdiScreen(None, 0);
        let mut shown: Option<crate::zakum::Line> = None;
        let mut seen: Option<std::time::Instant> = None;
        let started = std::time::Instant::now();
        loop {
            let round = std::time::Instant::now();
            let g = *gate.lock().unwrap();
            if let Some((t, p3, pct)) = g.filter(|g| Some(g.0) != seen) {
                seen = Some(t);
                zak.saw(p3, pct);
            }
            let show = if zak.active() && crate::boss::alerts(&shared.cfg.lock().unwrap()) {
                let (x, y, w, h) = platform::game_rect().unwrap_or_else(platform::primary_screen);
                let covers: Vec<crate::zakum::Rect> = platform::own_windows().into_iter().map(|r| (r.0 - x, r.1 - y, r.2 - x, r.3 - y)).filter(|r| r.0 < w && r.2 > 0 && r.1 < h && r.3 > 0).collect();
                let t = started.elapsed().as_secs_f64();
                screen
                    .grab(x, y, w, h)
                    .and_then(|img| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| zak.step(&img, t, &covers))).ok())
                    .map(|show| crate::zakum::Line { show, phase3: zak.phase3, face: zak.face_seen() })
            } else {
                None
            };
            if show != shown {
                shown = show;
                let _ = tx.send(Msg::Zakum(show));
                (repaint.lock().unwrap())();
            }
            let idle = if zak.active() { 250 } else { 1000 };
            std::thread::sleep(Duration::from_millis(idle).saturating_sub(round.elapsed()));
        }
    });
}

#[cfg(any(windows, target_os = "macos"))]
fn spawn_papu(paths: Paths, shared: Arc<Shared>, gate: Arc<Mutex<Option<(std::time::Instant, u32)>>>, tx: Sender<Msg>, repaint: Arc<Mutex<Box<dyn Fn() + Send>>>) {
    use crate::papulatus::{Clock, Marks, Spot};
    let log = std::fs::OpenOptions::new().create(true).append(true).open(paths.here.join("困拉偵測.log")).ok();
    let clock = Arc::new(Mutex::new(Clock::new(log.as_ref().and_then(|f| f.try_clone().ok()))));
    let seen: Arc<Mutex<Option<(Spot, std::time::Instant)>>> = Arc::new(Mutex::new(None));
    let drawn: Arc<Mutex<Option<Marks>>> = Arc::new(Mutex::new(None));
    let started = std::time::Instant::now();
    spawn_arrows(shared.clone(), gate.clone(), clock.clone(), seen.clone(), drawn.clone(), started);
    std::thread::spawn(move || {
        use crate::papulatus::{Bubbles, Foot, Mob, Mobs, FIRST_AT, FIRST_PCT, MOB_PCT};
        use reader::Screen;
        use std::io::Write;
        platform::below_normal_priority();
        let mut log = log;
        let mut foot = Foot::default();
        let bubbles = Bubbles::default();
        let mut mobs = Mobs::default();
        let mut mob_shown: Option<Mob> = None;
        let mut last_u: Option<f32> = None;
        let mut engine: Option<ocr::Ocr> = None;
        let mut screen = GdiScreen(None, 0);
        let mut shown: Option<crate::papulatus::Water> = None;
        let mut last_on: Option<std::time::Instant> = None;
        let mut size: Option<(i32, i32)> = None;
        let (mut high, mut armed, mut crossed) = (false, false, None);
        loop {
            let round = std::time::Instant::now();
            let pct = papu_now(&gate, &shared);
            if pct.is_some() && last_on.map_or(true, |t| t.elapsed().as_secs_f64() > 60.0) {
                foot.reset();
                lock(&clock).reset();
                mobs = Mobs::default();
                last_u = None;
                (high, armed, crossed) = (false, false, None);
                if let Some(f) = log.as_mut() {
                    let _ = writeln!(f, "{} 開始", tracker::local_iso().chars().take(23).collect::<String>());
                }
            }
            if pct.is_some() {
                last_on = Some(round);
            } else if last_on.map_or(false, |t| t.elapsed().as_secs_f64() > 60.0) {
                engine = None;
            }
            let mut water = None;
            let mut mob = None;
            if let Some(pct) = pct {
                let (x, y, w, h) = platform::game_rect().unwrap_or_else(platform::primary_screen);
                if size != Some((w, h)) {
                    size = Some((w, h));
                    last_u = None;
                }
                let t = started.elapsed().as_secs_f64();
                if pct > FIRST_PCT {
                    high = true;
                } else if high || pct >= FIRST_AT {
                    armed = true;
                }
                if armed && pct <= FIRST_AT && crossed.is_none() {
                    crossed = Some(t);
                }
                let mut spot = lock(&seen).filter(|s| pct <= 49 && s.1.elapsed().as_secs_f64() < 0.3).map(|s| s.0);
                let corner = spot.map_or(false, |s| !s.top(h as usize));
                let img = if corner { chat_corner(&mut screen, x, y, w, h) } else { screen.grab(x, y, w, h) };
                if let Some(img) = img {
                    let mask = lock(&drawn).as_ref().map_or(vec![], |m| m.area(x, y));
                    if pct <= 70 {
                        if engine.is_none() {
                            engine = ocr::Ocr::new(&paths.models.join(if cfg!(target_os = "macos") { "libonnxruntime.dylib" } else { "onnxruntime.dll" }), &paths.models).ok();
                        }
                        if let Some(e) = engine.as_mut() {
                            let mut said = String::new();
                            let mut rd = |c: &ocr::img::Bgr| {
                                let s = e.run(c, false, false).ok().and_then(|v| v.into_iter().next()).map(|it| it.text).unwrap_or_default();
                                said = s.clone();
                                s
                            };
                            let new = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| foot.step(&img, t, &mask, &mut rd))).unwrap_or(false);
                            if let Some(f) = log.as_mut().filter(|_| !said.is_empty() || new) {
                                let now = tracker::local_iso().chars().take(23).collect::<String>();
                                let _ = if said.is_empty() { writeln!(f, "{now} {pct}% 黑水（認得這行字）") } else { writeln!(f, "{now} {pct}% 讀到「{said}」{}", if new { " 黑水" } else { "" }) };
                            }
                        }
                    }
                    if pct <= MOB_PCT && spot.is_none() {
                        spot = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let mut clock = lock(&clock);
                            let near = clock.near(w as usize, h as usize);
                            near.and_then(|(rx, ry, rw, rh)| clock.track(&reader::cv::crop_bgr(&img, rx as usize, ry as usize, (rx + rw) as usize, (ry + rh) as usize), rx, ry, h as usize, t)).or_else(|| clock.find(&img, t))
                        }))
                        .ok()
                        .flatten();
                    }
                    if let Some(s) = &spot {
                        last_u = Some(s.u);
                    }
                    if let Some(u) = last_u.filter(|_| pct <= MOB_PCT) {
                        let zone = spot.as_ref().and_then(|s| s.spawn(w as usize, h as usize));
                        let look = spot.as_ref().map_or(true, |s| s.top(h as usize));
                        let (seen, born) = if look { std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bubbles.find(&img, u, zone))).unwrap_or((None, false)) } else { (None, false) };
                        if mobs.step(t, zone.is_some(), seen.is_some(), born) {
                            if let Some(f) = log.as_mut() {
                                let _ = writeln!(f, "{} {pct}% 時間怪出生（{:.2}）倒數 44 秒", tracker::local_iso().chars().take(23).collect::<String>(), seen.unwrap_or(0.0));
                            }
                        }
                    }
                }
                mob = mobs.show(t);
                if pct <= FIRST_PCT {
                    water = foot.show(t).or_else(|| foot.first(t, armed, crossed));
                }
            }
            if water != shown {
                shown = water;
                let _ = tx.send(Msg::Papu(water));
                (repaint.lock().unwrap())();
            }
            if mob != mob_shown {
                mob_shown = mob;
                let _ = tx.send(Msg::PapuMob(mob));
                (repaint.lock().unwrap())();
            }
            let idle = match pct {
                Some(p) if p <= 49 => 250,
                Some(p) if p <= 52 => 100,
                Some(_) => 250,
                None => 1000,
            };
            std::thread::sleep(Duration::from_millis(idle).saturating_sub(round.elapsed()));
        }
    });
}

#[cfg(any(windows, target_os = "macos"))]
fn spawn_arrows(shared: Arc<Shared>, gate: Arc<Mutex<Option<(std::time::Instant, u32)>>>, clock: Arc<Mutex<crate::papulatus::Clock>>, seen: Arc<Mutex<Option<(crate::papulatus::Spot, std::time::Instant)>>>, out: Arc<Mutex<Option<crate::papulatus::Marks>>>, started: std::time::Instant) {
    std::thread::spawn(move || {
        use crate::papulatus::{Marks, Spot};
        use reader::Screen;
        platform::below_normal_priority();
        let mut screen = GdiScreen(None, 0);
        let mut rect: Option<((i32, i32, i32, i32), std::time::Instant)> = None;
        let mut held: Option<Spot> = None;
        let mut drawn: Option<Marks> = None;
        let mut drawn_at = std::time::Instant::now();
        loop {
            let round = std::time::Instant::now();
            let fast = papu_now(&gate, &shared).map_or(false, |p| p <= 49);
            let mut marks = None;
            let mut wait = 100;
            if fast {
                if rect.map_or(true, |r| r.1.elapsed().as_millis() >= 250) {
                    let r = platform::game_rect().unwrap_or_else(platform::primary_screen);
                    if rect.map_or(false, |o| (o.0.2, o.0.3) != (r.2, r.3)) {
                        held = None;
                    }
                    rect = Some((r, round));
                }
                let (x, y, w, h) = rect.map(|r| r.0).unwrap_or_default();
                let near = lock(&clock).near(w as usize, h as usize);
                wait = if near.is_none() { 50 } else if cfg!(target_os = "macos") { 25 } else { 12 };
                if let Some((rx, ry, rw, rh)) = near {
                    let t = started.elapsed().as_secs_f64();
                    let spot = screen.grab(x + rx, y + ry, rw, rh).and_then(|img| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| lock(&clock).track(&img, rx, ry, h as usize, t))).ok().flatten());
                    if let Some(mut s) = spot {
                        *lock(&seen) = Some((s, round));
                        if let Some(p) = held.filter(|p| p.u == s.u && (p.x - s.x).abs() < 0.5 && (p.y - s.y).abs() < 0.5) {
                            s = p;
                        }
                        held = Some(s);
                        if let Some((xs, fl)) = s.marks(w as usize, h as usize) {
                            marks = Some(Marks { left: x, floor: y + fl.round() as i32, w, xs: xs.iter().map(|v| v.round() as i32).collect(), u: s.u });
                        }
                    }
                }
            }
            if marks.is_none() && fast && drawn_at.elapsed().as_secs_f64() < 0.5 {
                marks = drawn.clone();
            } else if marks.is_some() {
                drawn_at = round;
            }
            if marks != drawn {
                drawn = marks.clone();
                *lock(&out) = marks.clone();
                platform::marks(marks);
            }
            std::thread::sleep(Duration::from_millis(wait).saturating_sub(round.elapsed()));
        }
    });
}

#[cfg(any(windows, target_os = "macos"))]
fn papu_now(gate: &Mutex<Option<(std::time::Instant, u32)>>, shared: &Shared) -> Option<u32> {
    let g = gate.lock().unwrap().filter(|g| g.0.elapsed().as_secs_f64() < 20.0);
    g.filter(|_| crate::boss::alerts(&shared.cfg.lock().unwrap())).map(|g| g.1)
}

#[cfg(any(windows, target_os = "macos"))]
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(any(windows, target_os = "macos"))]
fn chat_corner(screen: &mut GdiScreen, x: i32, y: i32, w: i32, h: i32) -> Option<ocr::img::Bgr> {
    use reader::Screen;
    let (_, y0, x1, y1) = crate::papulatus::chat_area(w as usize, h as usize);
    let part = screen.grab(x, y + y0 as i32, x1 as i32, (y1 - y0) as i32)?;
    let mut img = ocr::img::Bgr::new(w as usize, h as usize);
    for r in 0..part.h {
        let d = (y0 + r) * img.w * 3;
        img.data[d..d + part.w * 3].copy_from_slice(&part.data[r * part.w * 3..(r + 1) * part.w * 3]);
    }
    Some(img)
}
