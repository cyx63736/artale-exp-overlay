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
    cfg.get("boss_mode").and_then(|v| v.as_bool()).unwrap_or(true)
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
        let same = region_of(cfg, "boss").map_or(false, |r| cfg.get("boss_screen").and_then(|v| v.as_str()) == Some(region_tag(r).as_str()));
        self.tick_in(cfg, engine, screen, area, same)
    }

    pub(crate) fn tick_in(&mut self, cfg: &Map<String, Value>, engine: &mut Ocr, screen: &mut dyn reader::Screen, area: (i32, i32, i32, i32), same: bool) -> Option<Out> {
        if let Some([x, y, w, h]) = region_of(cfg, "boss").filter(|r| same && r[1] - area.1 <= area.3 / TOP) {
            let Some(img) = screen.grab(x, y, w, h) else { return Some(Out::Read(None)) };
            let img = if h < SMALL { ocr::img::resize_linear(&img, img.w * 2, img.h * 2) } else { img };
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

const SMALL: i32 = 60;

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

fn total_range(r: &Reading) -> (f64, f64) {
    let hp = r.hp as f64 * 100.0;
    (hp / (r.pct + 1) as f64, if r.pct == 0 { f64::INFINITY } else { hp / r.pct as f64 })
}

fn same_name(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut d: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut prev = d[0];
        d[0] = i;
        for j in 1..=b.len() {
            let cur = d[j];
            d[j] = (prev + (a[i - 1] != b[j - 1]) as usize).min(d[j] + 1).min(d[j - 1] + 1);
            prev = cur;
        }
    }
    a == b || (d[b.len()] <= 1 && a.len().min(b.len()) >= 2)
}

const GONE: f64 = 90.0;

struct Fight {
    name: String,
    names: Vec<(String, usize)>,
    max: i64,
    total: (f64, f64),
    start_pct: u32,
    samples: Vec<(f64, i64)>,
    last_seen: f64,
    last_pct: u32,
    pending: Option<(f64, i64, u32)>,
    started: String,
}

impl Fight {
    fn new(now: f64, r: &Reading) -> Fight {
        let max = if r.pct == 0 { r.hp } else { ((r.hp as f64 * 100.0 / r.pct as f64).round() as i64).max(r.hp) };
        Fight { name: r.name.clone(), names: vec![(r.name.clone(), 1)], max, total: total_range(r), start_pct: r.pct, samples: vec![(now, r.hp)], last_seen: now, last_pct: r.pct, pending: None, started: tracker::local_iso().chars().take(19).collect::<String>().replace('T', " ") }
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
    fn rate(&self) -> Option<f64> {
        let e = self.elapsed();
        self.recent().or_else(|| (e > 0.0).then(|| (self.samples[0].1 - self.last().1) as f64 / e)).filter(|&d| d > 0.0)
    }
    fn dead(&self) -> bool {
        let lh = self.last().1;
        self.last_pct <= 1 || lh * 100 <= self.max || self.rate().map_or(false, |d| lh as f64 / d <= 5.0)
    }
    fn take(&mut self, now: f64, r: &Reading) -> bool {
        let last = self.last().1;
        if r.hp > last || !self.plausible(r) {
            return false;
        }
        let big = last - r.hp > self.max / 10;
        if big && !self.pending.map_or(false, |p| r.hp <= p.1 + self.max / 50) {
            self.pending = Some((now, r.hp, r.pct));
            return false;
        }
        self.samples.push((now, r.hp));
        self.last_pct = r.pct;
        self.pending = None;
        match self.names.iter_mut().find(|n| n.0 == r.name) {
            Some(n) => n.1 += 1,
            None => self.names.push((r.name.clone(), 1)),
        }
        self.name = self.names.iter().max_by_key(|n| n.1).unwrap().0.clone();
        let (lo, hi) = total_range(r);
        let t = (self.total.0.max(lo), self.total.1.min(hi));
        if t.0 <= t.1 {
            self.total = t;
            if t.1.is_finite() {
                self.max = ((t.0 + t.1) / 2.0).round() as i64;
            }
        }
        true
    }
    fn settle(&mut self) {
        if let Some((t, hp, pct)) = self.pending.take() {
            self.samples.push((t, hp));
            self.last_pct = pct;
        }
    }
    fn plausible(&self, r: &Reading) -> bool {
        if self.max <= 0 {
            return true;
        }
        (r.hp as f64 * 100.0 / self.max as f64 - r.pct as f64).abs() <= 3.0
    }
}

const CURVE_STEP: f64 = 10.0;

fn curve_of(samples: &[(f64, i64)], secs: f64) -> Vec<(f64, i64)> {
    let (t0, h0) = samples[0];
    let hp_at = |t: f64| -> f64 {
        let i = samples.partition_point(|s| s.0 <= t);
        if i == 0 {
            return h0 as f64;
        }
        if i >= samples.len() {
            return samples[samples.len() - 1].1 as f64;
        }
        let (a, b) = (samples[i - 1], samples[i]);
        a.1 as f64 + (b.1 - a.1) as f64 * (t - a.0) / (b.0 - a.0)
    };
    let mut v = vec![];
    let mut s = 0.0;
    while s < secs - 1.0 {
        v.push((s, (h0 as f64 - hp_at(t0 + s)).round() as i64));
        s += CURVE_STEP;
    }
    v.push((secs, h0));
    v
}

#[derive(Default)]
pub struct Boss {
    fight: Option<Fight>,
    cand: Option<(f64, Reading)>,
    ended: Option<(f64, String)>,
}

pub fn on_msg(o: &mut Overlay, m: Out) {
    let now = o.now();
    match m {
        Out::Found(r, rd) => {
            o.t.cfg.insert("boss_region".into(), serde_json::json!(r));
            o.t.cfg.insert("boss_screen".into(), region_tag(r).into());
            o.save_cfg();
            o.t.logs.push(format!("找到王的血條：{}", rd.name));
            o.t.say("找到王的血條，開始算傷害", 5.0);
            feed(o, now, Some(rd));
        }
        Out::Read(rd) => feed(o, now, rd),
    }
}

impl Boss {
    fn step(&mut self, now: f64, rd: Option<Reading>) -> Option<Fight> {
        match rd {
            Some(r) => {
                if let Some(f) = self.fight.as_mut() {
                    let last = f.last().1;
                    if r.hp <= last + f.max / 20 {
                        if r.pct < 90 || f.plausible(&r) {
                            self.cand = None;
                            f.last_seen = now;
                            f.take(now, &r);
                            return None;
                        }
                        match self.cand.take() {
                            Some((t, c)) if Fight::new(t, &c).plausible(&r) => {
                                let mut nf = Fight::new(t, &c);
                                nf.last_seen = now;
                                nf.take(now, &r);
                                return self.fight.replace(nf);
                            }
                            _ => {
                                self.cand = Some((now, r));
                                return None;
                            }
                        }
                    }
                }
                self.cand = None;
                self.fight.replace(Fight::new(now, &r))
            }
            None => {
                let f = self.fight.as_mut()?;
                let gap = now - f.last_seen;
                if gap > 4.0 {
                    f.settle();
                    if f.dead() || gap > GONE {
                        return self.fight.take();
                    }
                }
                None
            }
        }
    }
}

fn feed(o: &mut Overlay, now: f64, rd: Option<Reading>) {
    let fresh = rd.is_some();
    if let Some(f) = o.boss.step(now, rd) {
        finish(o, f);
    }
    if fresh {
        o.boss.ended = None;
    }
}

fn kill_secs(f: &Fight) -> Option<f64> {
    if f.samples.len() < 2 || f.elapsed() <= 0.0 || !f.dead() {
        return None;
    }
    Some(f.elapsed() + f.rate().map_or(0.0, |d| f.last().1 as f64 / d))
}

fn finish(o: &mut Overlay, mut f: Fight) {
    f.settle();
    if f.samples.len() < 2 || f.elapsed() <= 0.0 {
        return;
    }
    let Some(secs) = kill_secs(&f) else {
        o.t.logs.push(format!("{} 的血條不見了（打到 {}%），這次不記", f.name, f.last_pct));
        write_wipe(&o.paths.here.join(crate::history::BOSS_CURVE_FILE), &f);
        return;
    };
    if let Some(n) = crate::history::raid_name(f.max) {
        f.name = n.into();
    }
    let dmg = f.samples[0].1;
    write_kill(&o.paths.here.join(crate::history::BOSS_FILE), &f, secs);
    let msg = format!("擊殺{} 耗時 {}，DPM {}", f.name, ms(secs), short(Some(dmg as f64 / secs * 60.0)));
    o.t.logs.push(format!("打王：{msg}（從 {}% 開始）", f.start_pct));
    o.boss.ended = Some((o.now(), msg));
}

fn write_wipe(path: &std::path::Path, f: &Fight) {
    let line = serde_json::json!({ "when": f.started, "name": f.name, "wipe": true }).to_string() + "
";
    if let Ok(mut fh) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = fh.write_all(line.as_bytes());
    }
}

fn write_kill(path: &std::path::Path, f: &Fight, secs: f64) {
    let dmg = f.samples[0].1;
    let dps = dmg as f64 / secs;
    let new = std::fs::metadata(path).map_or(true, |m| m.len() == 0);
    if let Ok(mut fh) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        if new {
            let _ = write!(fh, "\u{feff}{}\r\n", crate::history::BOSS_HEAD.join(","));
        }
        let _ = write!(fh, "{},{},{},{},{secs:.0},{dmg},{dps:.0},{:.0}\r\n", f.started, f.name, f.max, f.start_pct, dps * 60.0);
    }
    let pts: Vec<Value> = curve_of(&f.samples, secs).iter().map(|&(t, d)| serde_json::json!([(t * 10.0).round() / 10.0, d])).collect();
    let line = serde_json::json!({ "when": f.started, "name": f.name, "secs": secs.round() as i64, "pts": pts }).to_string() + "\n";
    if let Ok(mut fh) = std::fs::OpenOptions::new().create(true).append(true).open(path.with_file_name(crate::history::BOSS_CURVE_FILE)) {
        let _ = fh.write_all(line.as_bytes());
    }
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
        let mut v = vec![
            ("血量".into(), HP, format!("{} {}（{}%）", crate::history::raid_name(f.max).unwrap_or(&f.name), comma(lh), f.last_pct)),
            ("DPM".into(), EXP, format!("{} 平均 {}", per_min(f.recent()), per_min(f.avg()))),
            ("預估時間".into(), GOOD, format!("{}（已用 {}）", rate.map_or("–".into(), |d| ms(lh as f64 / d)), ms(f.elapsed()))),
        ];
        if let Some((z, txt)) = o.zakum.and_then(|z| z.text().map(|t| (z, t))) {
            let c = match z.show {
                crate::zakum::Show::Next | crate::zakum::Show::Late => HP,
                crate::zakum::Show::Left(_) => WARN,
                crate::zakum::Show::Wait => GOOD,
            };
            v.push(("黑水".into(), c, txt));
        }
        return v;
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

    fn rd(hp: i64, pct: u32) -> Reading {
        Reading { name: "王".into(), hp, pct }
    }

    #[test]
    fn low_hp_boss() {
        let mut f = Fight::new(0.0, &rd(1_000_000, 100));
        assert!(!f.take(1.0, &rd(750_000, 75)));
        assert!(f.take(2.0, &rd(500_000, 50)));
        assert!(!f.take(3.0, &rd(250_000, 25)));
        f.settle();
        assert_eq!(f.last(), (3.0, 250_000));
        assert!(f.dead());
    }

    #[test]
    fn misread_drop() {
        let mut f = Fight::new(0.0, &rd(64_906_862, 99));
        assert!(f.take(1.0, &rd(64_785_127, 99)));
        assert!(!f.take(2.0, &rd(685_127, 1)));
        assert!(f.take(3.0, &rd(64_600_000, 99)));
        assert!(f.pending.is_none());
        f.settle();
        assert_eq!(f.last().1, 64_600_000);
        assert!(!f.dead());
    }

    #[test]
    fn curve_every_10s() {
        let s: Vec<(f64, i64)> = (0..12).map(|i| (100.0 + i as f64 * 3.0, 3500 - i as i64 * 300)).collect();
        let c = curve_of(&s, 35.0);
        assert_eq!(c, vec![(0.0, 0), (10.0, 1000), (20.0, 2000), (30.0, 3000), (35.0, 3500)]);
    }

    #[test]
    fn total_narrows() {
        let real = 66_000_000i64;
        let rd = |hp: i64| Reading { name: "炎魔".into(), hp, pct: (hp * 100 / real) as u32 };
        let mut f = Fight::new(0.0, &rd(65_545_745));
        for (i, hp) in (1..60).map(|i| 65_545_745 - i * 1_090_000).enumerate() {
            f.take(i as f64 + 1.0, &rd(hp));
        }
        assert!((f.max - real).abs() * 1000 < real, "{}", f.max);
    }

    #[test]
    fn names_off_by_one() {
        assert!(same_name("闇黑龍王", "黑龍王"));
        assert!(same_name("闇黑龍王", "間黑龍王"));
        assert!(same_name("拉圖斯", "拉圖斯"));
        assert!(!same_name("拉圖斯", "拉圖斯的時鐘"));
        assert!(!same_name("殭屍菇菇王", "藍色菇菇王"));
        assert!(!same_name("王", "玉"));
    }

    #[test]
    fn full_bar_of_another_boss() {
        let rd = |hp: i64, pct: u32| Reading { name: "闇黑龍王".into(), hp, pct };
        let mut b = Boss::default();
        b.step(0.0, Some(rd(1_848_000_000, 100)));
        b.step(60.0, Some(rd(1_663_200_000, 90)));
        assert!(b.step(70.0, Some(rd(134_860_000, 100))).is_none());
        let old = b.step(71.0, Some(rd(134_000_000, 99))).expect("前一場結束");
        assert!(old.max > 1_800_000_000);
        let f = b.fight.as_ref().unwrap();
        assert_eq!((f.samples[0], f.samples.len()), ((70.0, 134_860_000), 2));
        let mut b = Boss::default();
        b.step(0.0, Some(rd(64_785_127, 100)));
        b.step(1.0, Some(rd(6_478_512, 99)));
        assert!(b.step(2.0, Some(rd(64_000_000, 98))).is_none());
        assert_eq!(b.fight.as_ref().unwrap().samples.len(), 2);
    }

    #[test]
    fn one_row_each() {
        let dir = std::env::temp_dir().join(format!("boss_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(crate::history::BOSS_FILE);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(dir.join(crate::history::BOSS_CURVE_FILE));
        let fight = |when: &str, name: &str, max: i64, secs: f64| Fight { name: name.into(), names: vec![], max, total: (0.0, f64::INFINITY), start_pct: 100, samples: vec![(0.0, max), (secs, 0)], last_seen: secs, last_pct: 0, pending: None, started: when.into() };
        write_kill(&path, &fight("2026-10-12 11:00:05", "拉圖斯", 53_750_000, 400.0), 400.0);
        write_kill(&path, &fight("2026-10-12 11:06:52", "拉圖斯", 12_450_310, 100.0), 100.0);
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3, "{text}");
        assert_eq!(lines[2], "2026-10-12 11:06:52,拉圖斯,12450310,100,100,12450310,124503,7470186");
        let recs = crate::history::load_boss(&path);
        assert_eq!(recs.len(), 2);
        assert_eq!((recs[1].name.as_str(), recs[1].max, recs[1].secs), ("拉圖斯", 12_450_310, 100.0));
        assert_eq!(std::fs::read_to_string(dir.join(crate::history::BOSS_CURVE_FILE)).unwrap().lines().count(), 2);
        let _ = std::fs::remove_dir_all(&dir);
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
            let out = w.tick_in(&cfg, &mut engine, &mut scr, (0, 0, 3840, 2160), true);
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
                    if !f.take(now, &r) {
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
        let mut f = fight.unwrap();
        f.settle();
        assert!(f.dead());
        if let Ok(p) = std::env::var("BOSS_CSV") {
            let lh = f.last().1;
            let secs = f.elapsed() + f.rate().map_or(0.0, |d| lh as f64 / d);
            let when = std::env::var("BOSS_WHEN").unwrap_or("2026-10-11 21:00".into());
            f.started = when;
            write_kill(std::path::Path::new(&p), &f, secs);
        }
        println!("frames {} missing {miss} dropped {drop} samples {} max {} start {}% end {}% elapsed {} avg {:?}", files.len(), f.samples.len(), f.max, f.start_pct, f.last_pct, f.elapsed(), f.avg());
    }

    #[test]
    #[ignore]
    fn boss_video() {
        struct Fake(Option<ocr::img::Bgr>);
        impl reader::Screen for Fake {
            fn grab(&mut self, x: i32, y: i32, w: i32, h: i32) -> Option<ocr::img::Bgr> {
                let img = self.0.as_ref()?;
                if x < 0 || y < 0 || (x + w) as usize > img.w {
                    return None;
                }
                let mut o = ocr::img::Bgr::new(w as usize, h as usize);
                for r in 0..(h as usize).min(img.h.saturating_sub(y as usize)) {
                    let src = ((y as usize + r) * img.w + x as usize) * 3;
                    o.data[r * o.w * 3..(r + 1) * o.w * 3].copy_from_slice(&img.data[src..src + o.w * 3]);
                }
                Some(o)
            }
        }
        let dir = std::path::PathBuf::from(std::env::var("BOSS_FRAMES").unwrap());
        let models = std::path::PathBuf::from(std::env::var("BOSS_MODELS").unwrap());
        let csv = std::env::var("BOSS_CSV").ok();
        let when = std::env::var("BOSS_WHEN").unwrap_or("2026-10-12 20:00:00".into());
        let base = when[11..13].parse::<i64>().unwrap() * 3600 + when[14..16].parse::<i64>().unwrap() * 60 + when[17..19].parse::<i64>().unwrap();
        let clock = |t: f64| {
            let s = base + t as i64;
            format!("{} {:02}:{:02}:{:02}", &when[..10], s / 3600 % 24, s / 60 % 60, s % 60)
        };
        let mut engine = Ocr::new(&models.join("onnxruntime.dll"), &models).unwrap();
        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.sort();
        let mut cfg = Map::new();
        let mut w = Watcher::default();
        let mut boss = Boss::default();
        let mut last = 0.0;
        let mut done = |f: Fight| {
            let mut f = f;
            f.settle();
            let from = f.samples[0].0;
            if let Some(n) = crate::history::raid_name(f.max) {
                f.name = n.into();
            }
            match kill_secs(&f) {
                Some(secs) => {
                    println!("{} 擊殺 {} 血量 {} 從 {}% 開始，耗時 {}，DPM {}", clock(from), f.name, f.max, f.start_pct, ms(secs), short(Some(f.samples[0].1 as f64 / secs * 60.0)));
                    if let Some(p) = &csv {
                        f.started = clock(from);
                        write_kill(std::path::Path::new(p), &f, secs);
                    }
                }
                None => {
                    println!("{} {} 血量 {} 打到 {}% 不見了，不記（{} 次讀數）", clock(from), f.name, f.max, f.last_pct, f.samples.len());
                    if let Some(p) = csv.as_ref().filter(|_| f.samples.len() >= 2) {
                        f.started = clock(from);
                        write_wipe(&std::path::Path::new(p).with_file_name(crate::history::BOSS_CURVE_FILE), &f);
                    }
                }
            }
        };
        for f in &files {
            let now: f64 = f.file_stem().unwrap().to_string_lossy().parse().unwrap();
            let img = ocr::img::Bgr::load(f).ok();
            let area = img.as_ref().map_or((0, 0, 0, 0), |i| (0, 0, i.w as i32, i.h as i32 * TOP));
            let mut scr = Fake(img);
            w.last_search = None;
            let r = match w.tick_in(&cfg, &mut engine, &mut scr, area, true) {
                Some(Out::Found(reg, r)) => {
                    println!("{} 找到血條 {reg:?} {r:?}", clock(now));
                    cfg.insert("boss_region".into(), serde_json::json!(reg));
                    Some(r)
                }
                Some(Out::Read(r)) => r,
                None => None,
            };
            if std::env::var("BOSS_TRACE").is_ok() {
                println!("{now:>5} {r:?}");
            }
            if let Some(f) = boss.step(now, r) {
                done(f);
            }
            last = now;
        }
        if let Some(f) = boss.step(last + GONE + 1.0, None) {
            done(f);
        }
    }
}
