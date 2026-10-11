use crate::ui::*;
use egui::Color32;
use ocr::Ocr;
use serde_json::{Map, Value};
use std::io::Write;
use std::time::{Duration, Instant};
use tracker::util::{comma, short};

#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub name: String,
    pub hp: i64,
    pub pct: u32,
}

pub fn parse(s: &str) -> Option<Reading> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).map(|c| match c {
        '（' => '(',
        '）' => ')',
        '，' => ',',
        c => c,
    }).collect();
    let open = s.rfind('(')?;
    let rest = &s[open + 1..];
    let pct_s: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if pct_s.is_empty() || !rest[pct_s.len()..].starts_with('%') {
        return None;
    }
    let pct: u32 = pct_s.parse().ok().filter(|&p| p <= 100)?;
    let head = &s[..open];
    let num_at = head.char_indices().rev().take_while(|(_, c)| c.is_ascii_digit() || *c == ',' || *c == '.').last().map(|(i, _)| i)?;
    let digits: String = head[num_at..].chars().filter(|c| c.is_ascii_digit()).collect();
    let name = head[..num_at].to_string();
    if name.is_empty() || digits.is_empty() || digits.len() > 13 {
        return None;
    }
    Some(Reading { name, hp: digits.parse().ok()?, pct })
}

pub fn enabled(cfg: &Map<String, Value>) -> bool {
    cfg.get("boss_mode").and_then(|v| v.as_bool()).unwrap_or(false)
}

pub enum Out {
    Found([i32; 4], Reading),
    Read(Option<Reading>),
}

#[derive(Default)]
pub struct Watcher {
    last_search: Option<Instant>,
}

impl Watcher {
    pub fn tick(&mut self, cfg: &Map<String, Value>, engine: &mut Ocr, screen: &mut dyn reader::Screen) -> Option<Out> {
        #[cfg(not(target_os = "macos"))]
        let area = region_of(cfg, "exp").and_then(|r| monitor(&crate::platform::screen_tag(r[0] + r[2] / 2, r[1] + r[3] / 2))).unwrap_or_else(crate::platform::virtual_screen);
        #[cfg(target_os = "macos")]
        let area = region_of(cfg, "exp").map_or_else(crate::platform::virtual_screen, |r| crate::platform::monitor_at(r[0] + r[2] / 2, r[1] + r[3] / 2));
        self.tick_in(cfg, engine, screen, area)
    }

    fn tick_in(&mut self, cfg: &Map<String, Value>, engine: &mut Ocr, screen: &mut dyn reader::Screen, area: (i32, i32, i32, i32)) -> Option<Out> {
        if let Some([x, y, w, h]) = region_of(cfg, "boss").filter(|r| r[1] - area.1 <= area.3 / TOP) {
            let Some(img) = screen.grab(x, y, w, h) else { return Some(Out::Read(None)) };
            let items = engine.run(&img, true, false).ok()?;
            return Some(Out::Read(items.iter().find_map(|it| parse(&it.text))));
        }
        if self.last_search.map_or(false, |t| t.elapsed() < Duration::from_secs(3)) {
            return None;
        }
        self.last_search = Some(Instant::now());
        let (l, t, w, h) = area;
        let img = screen.grab(l, t, w, h / TOP)?;
        for it in engine.run(&img, true, false).ok()? {
            let (Some(r), Some(q)) = (parse(&it.text), it.quad) else { continue };
            let (x0, y0) = (q.iter().map(|p| p[0]).fold(f32::MAX, f32::min), q.iter().map(|p| p[1]).fold(f32::MAX, f32::min));
            let (x1, y1) = (q.iter().map(|p| p[0]).fold(f32::MIN, f32::max), q.iter().map(|p| p[1]).fold(f32::MIN, f32::max));
            let bh = y1 - y0;
            let rx = (x0 - bh * 0.6).max(0.0);
            let ry = (y0 - bh * 0.5).max(0.0);
            let rw = (x1 - x0) * 1.3 + bh * 1.2;
            let rh = bh * 2.0;
            return Some(Out::Found([l + rx as i32, t + ry as i32, rw as i32, rh as i32], r));
        }
        None
    }
}

#[cfg(not(target_os = "macos"))]
const TOP: i32 = 10;
#[cfg(target_os = "macos")]
const TOP: i32 = 6;

#[cfg(not(target_os = "macos"))]
fn monitor(tag: &str) -> Option<(i32, i32, i32, i32)> {
    let mut p = tag.split(',');
    let (l, t) = (p.next()?.parse().ok()?, p.next()?.parse().ok()?);
    let (w, h) = p.next()?.split_once('x')?;
    Some((l, t, w.parse().ok()?, h.parse().ok()?))
}

const GONE: f64 = 90.0;

struct Fight {
    name: String,
    max: i64,
    start_pct: u32,
    samples: Vec<(f64, i64)>,
    last_seen: f64,
    last_pct: u32,
}

impl Fight {
    fn new(now: f64, r: &Reading) -> Fight {
        let max = if r.pct == 0 { r.hp } else { ((r.hp as f64 * 100.0 / r.pct as f64).round() as i64).max(r.hp) };
        Fight { name: r.name.clone(), max, start_pct: r.pct, samples: vec![(now, r.hp)], last_seen: now, last_pct: r.pct }
    }
    fn last(&self) -> (f64, i64) {
        *self.samples.last().unwrap()
    }
    fn elapsed(&self) -> f64 {
        self.last().0 - self.samples[0].0
    }
    fn avg(&self) -> Option<f64> {
        let e = self.elapsed();
        (e >= 3.0).then(|| (self.samples[0].1 - self.last().1) as f64 / e)
    }
    fn recent(&self) -> Option<f64> {
        let (lt, lh) = self.last();
        let &(t0, h0) = self.samples.iter().find(|s| s.0 >= lt - 10.0)?;
        (lt - t0 >= 3.0).then(|| (h0 - lh) as f64 / (lt - t0))
    }
    fn plausible(&self, r: &Reading) -> bool {
        if self.max <= 0 || !(3..=97).contains(&r.pct) {
            return true;
        }
        (r.hp as f64 * 100.0 / self.max as f64 - r.pct as f64).abs() <= 3.0
    }
}

#[derive(Default)]
pub struct Boss {
    fight: Option<Fight>,
    ended: Option<(f64, String)>,
}

pub fn on_msg(o: &mut Overlay, m: Out) {
    let now = o.now();
    match m {
        Out::Found(r, rd) => {
            o.t.cfg.insert("boss_region".into(), serde_json::json!(r));
            o.save_cfg();
            o.t.logs.push(format!("找到王的血條：{}", rd.name));
            o.t.say("找到王的血條，開始算傷害", 5.0);
            feed(o, now, Some(rd));
        }
        Out::Read(rd) => feed(o, now, rd),
    }
}

fn feed(o: &mut Overlay, now: f64, rd: Option<Reading>) {
    match rd {
        Some(r) => {
            if let Some(f) = o.boss.fight.as_mut() {
                let last = f.last().1;
                if r.hp <= last + f.max / 20 && (r.name == f.name || r.pct < 90) {
                    f.last_seen = now;
                    if r.name == f.name && r.hp <= last && f.plausible(&r) {
                        f.samples.push((now, r.hp));
                        f.last_pct = r.pct;
                    }
                    return;
                }
                finish(o);
            }
            o.boss.ended = None;
            o.boss.fight = Some(Fight::new(now, &r));
        }
        None => {
            if o.boss.fight.as_ref().map_or(false, |f| now - f.last_seen > if f.last_pct <= 1 { 4.0 } else { GONE }) {
                finish(o);
            }
        }
    }
}

fn finish(o: &mut Overlay) {
    let Some(f) = o.boss.fight.take() else { return };
    let (_, lh) = f.last();
    let e = f.elapsed();
    if f.samples.len() < 3 || e < 5.0 {
        return;
    }
    let killed = f.last_pct <= 1 || lh * 100 <= f.max;
    if !killed {
        o.t.logs.push(format!("{} 的血條不見了（打到 {}%），這次不記", f.name, f.last_pct));
        return;
    }
    let dmg = f.samples[0].1 - lh;
    let dps = dmg as f64 / e;
    let when = tracker::local_iso().chars().take(16).collect::<String>().replace('T', " ");
    let path = o.paths.here.join("打王紀錄.csv");
    let new = !path.exists();
    if let Ok(mut fh) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        if new {
            let _ = write!(fh, "\u{feff}時間,王,總血量,開始%,已用（秒）,傷害,每秒傷害,DPM\r\n");
        }
        let _ = write!(fh, "{when},{},{},{},{:.0},{dmg},{:.0},{:.0}\r\n", f.name, f.max, f.start_pct, e, dps, dps * 60.0);
    }
    let msg = format!("擊殺{} 耗時 {}，DPM {}", f.name, ms(e), short(Some(dps * 60.0)));
    o.t.logs.push(format!("打王：{msg}（從 {}% 開始）", f.start_pct));
    o.boss.ended = Some((o.now(), msg));
}

fn ms(sec: f64) -> String {
    let s = sec.max(0.0).round() as i64;
    format!("{}:{:02}", s / 60, s % 60)
}

pub fn rows(o: &Overlay) -> Vec<(String, Color32, String)> {
    if !enabled(&o.t.cfg) {
        return vec![];
    }
    let now = o.now();
    if let Some(f) = o.boss.fight.as_ref().filter(|f| now - f.last_seen <= GONE) {
        let (_, lh) = f.last();
        let rate = f.recent().or(f.avg()).filter(|&d| d > 0.0);
        let per_min = |v: Option<f64>| short(v.map(|d| d * 60.0));
        return vec![
            ("血量".into(), HP, format!("{} {}（{}%）", f.name, comma(lh), f.last_pct)),
            ("DPM".into(), EXP, format!("{} 平均 {}", per_min(f.recent()), per_min(f.avg()))),
            ("預估時間".into(), GOOD, format!("{}（已用 {}）", rate.map_or("–".into(), |d| ms(lh as f64 / d)), ms(f.elapsed()))),
        ];
    }
    match &o.boss.ended {
        Some((t, msg)) if now - t < 30.0 => vec![(String::new(), GOOD, msg.clone())],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bar() {
        let r = |n: &str, hp, pct| Some(Reading { name: n.into(), hp, pct });
        assert_eq!(parse("海怒斯 49,945,312 (76%)"), r("海怒斯", 49_945_312, 76));
        assert_eq!(parse("海怒斯50,129,489（77%)"), r("海怒斯", 50_129_489, 77));
        assert_eq!(parse("海怒斯76,000（0%）"), r("海怒斯", 76_000, 0));
        assert_eq!(parse("海怒斯23，065，620（35%）"), r("海怒斯", 23_065_620, 35));
        assert_eq!(parse("EXP 102150884[50.36%]"), None);
        assert_eq!(parse("49,945,312 (76%)"), None);
    }

    #[test]
    #[ignore]
    fn boss_frames() {
        struct Fake(Option<ocr::img::Bgr>);
        impl reader::Screen for Fake {
            fn grab(&mut self, x: i32, y: i32, w: i32, h: i32) -> Option<ocr::img::Bgr> {
                let img = self.0.as_ref()?;
                let (x1, y1) = ((x + w) as usize, (y + h) as usize);
                (x >= 0 && y >= 0 && x1 <= img.w && y1 <= img.h).then(|| reader::cv::crop_bgr(img, x as usize, y as usize, x1, y1))
            }
        }
        let dir = std::path::PathBuf::from(std::env::var("BOSS_FRAMES").unwrap());
        let models = std::path::PathBuf::from(std::env::var("BOSS_MODELS").unwrap());
        let mut engine = Ocr::new(&models.join("onnxruntime.dll"), &models).unwrap();
        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.sort();
        let mut cfg = Map::new();
        let mut w = Watcher::default();
        let mut fight: Option<Fight> = None;
        let (mut miss, mut drop) = (0, 0);
        for (i, f) in files.iter().enumerate() {
            let now = i as f64 * 3.0;
            let mut scr = Fake(ocr::img::Bgr::load(f).ok());
            w.last_search = None;
            let out = w.tick_in(&cfg, &mut engine, &mut scr, (0, 0, 3840, 2160));
            let r = match out {
                Some(Out::Found(reg, r)) => {
                    println!("found at {now}s region {reg:?} {r:?}");
                    cfg.insert("boss_region".into(), serde_json::json!(reg));
                    Some(r)
                }
                Some(Out::Read(r)) => r,
                None => None,
            };
            let Some(r) = r else {
                miss += 1;
                continue;
            };
            match fight.as_mut() {
                None => fight = Some(Fight::new(now, &r)),
                Some(f) => {
                    if r.hp <= f.last().1 && f.plausible(&r) {
                        f.samples.push((now, r.hp));
                        f.last_pct = r.pct;
                    } else {
                        drop += 1;
                        println!("drop {now}s {r:?}");
                    }
                }
            }
            if let Some(f) = &fight {
                if i % 10 == 0 {
                    println!("{now:>4}s {} {}% recent {:?} avg {:?}", f.last().1, f.last_pct, f.recent().map(|d| d as i64), f.avg().map(|d| d as i64));
                }
            }
        }
        let f = fight.unwrap();
        println!("frames {} missing {miss} dropped {drop} samples {} max {} start {}% end {}% elapsed {} avg {:?}", files.len(), f.samples.len(), f.max, f.start_pct, f.last_pct, f.elapsed(), f.avg());
    }
}
