use crate::platform;
use crate::ui::*;
use egui::{self, pos2, vec2, Align, Align2, Color32, Layout, Pos2, Rect, RichText, Sense, Shape, Stroke, ViewportCommand};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracker::util::{comma, short};
use tracker::CurvePt;

pub const TITLE: &str = "紀錄";

#[derive(Clone, Debug, PartialEq)]
pub struct Seg {
    pub start: String,
    pub min: f64,
    pub exp: i64,
    pub levelups: i64,
    pub used: [i64; 2],
    pub cost: f64,
    pub meso: Option<i64>,
    pub sold: i64,
    pub names: [String; 2],
    pub extra: String,
    pub net: Option<f64>,
    pub live: bool,
}

impl Seg {
    fn day(&self) -> &str {
        self.start.get(..10).unwrap_or(&self.start)
    }
    fn hm(&self) -> &str {
        self.start.get(11..16).unwrap_or("")
    }
}

fn parse_row(line: &str) -> Option<Seg> {
    let c: Vec<&str> = line.trim_end_matches(['\r', '\n']).split(',').map(|s| s.trim().trim_matches('"')).collect();
    if c.len() < 9 {
        return None;
    }
    let start = c[0].to_string();
    if start.len() < 16 || !start.as_bytes()[0].is_ascii_digit() {
        return None;
    }
    let f = |i: usize| c.get(i).and_then(|s| s.parse::<f64>().ok());
    let n = |i: usize| f(i).map(|v| v.round() as i64);
    let cost = f(8).unwrap_or(0.0);
    let meso = n(11);
    let net = f(12).or(meso.map(|m| m as f64 - cost));
    Some(Seg {
        start,
        min: f(1)?,
        exp: n(2)?,
        levelups: n(4).unwrap_or(0),
        used: [n(6).unwrap_or(0), n(7).unwrap_or(0)],
        cost,
        meso,
        sold: n(14).unwrap_or(0),
        names: [c.get(15).unwrap_or(&"").to_string(), c.get(16).unwrap_or(&"").to_string()],
        extra: c.get(17).unwrap_or(&"").to_string(),
        net,
        live: false,
    })
}

fn csv_files(csv: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = vec![csv.to_path_buf()];
    if let (Some(dir), Some(stem)) = (csv.parent(), csv.file_stem().map(|s| s.to_string_lossy().to_string())) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with(&format!("{stem}_")) && name.ends_with(".csv") {
                    files.push(e.path());
                }
            }
        }
    }
    files
}

pub fn load(csv: &Path) -> Vec<Seg> {
    let mut segs: Vec<Seg> = vec![];
    for p in csv_files(csv) {
        let Ok(raw) = std::fs::read(&p) else { continue };
        let text = String::from_utf8_lossy(raw.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&raw)).to_string();
        for line in text.lines() {
            if let Some(s) = parse_row(line) {
                if !segs.iter().any(|o| o.start == s.start && o.exp == s.exp) {
                    segs.push(s);
                }
            }
        }
    }
    segs.sort_by(|a, b| a.start.cmp(&b.start));
    segs
}

pub fn load_curves(csv: &Path) -> HashMap<String, Vec<CurvePt>> {
    let mut m = HashMap::new();
    let Ok(text) = std::fs::read_to_string(csv.with_file_name(tracker::CURVE_FILE)) else { return m };
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        if let Some(start) = v["start"].as_str() {
            let pts = tracker::parse_curve(&v["pts"]);
            if pts.len() >= 2 {
                m.insert(start.to_string(), pts);
            }
        }
    }
    m
}

pub const BOSS_FILE: &str = "打王紀錄.csv";
pub const BOSS_CURVE_FILE: &str = "打王曲線.jsonl";
pub const BOSS_HEAD: [&str; 8] = ["時間", "王", "總血量", "開始%", "已用（秒）", "傷害", "每秒傷害", "DPM"];

#[derive(Clone, Debug, PartialEq)]
pub struct BossRec {
    pub when: String,
    pub name: String,
    pub max: i64,
    pub secs: f64,
    pub dmg: i64,
    pub dpm: f64,
    pub start: f64,
}

pub fn parse_boss(line: &str) -> Option<BossRec> {
    let c: Vec<&str> = line.trim_end_matches(['\r', '\n']).split(',').map(|s| s.trim().trim_matches('"')).collect();
    if c.len() < 8 || c[0].len() < 16 || !c[0].as_bytes()[0].is_ascii_digit() {
        return None;
    }
    Some(BossRec {
        when: c[0].to_string(),
        name: c[1].to_string(),
        max: c[2].parse().ok()?,
        secs: c[4].parse().ok()?,
        dmg: c[5].parse().ok()?,
        dpm: c[7].parse().ok()?,
        start: secs_of(c[0]),
    })
}

fn secs_of(when: &str) -> f64 {
    let d = days_of(when.get(..10).unwrap_or("")).unwrap_or(0) as f64;
    let n = |a: usize, b: usize| when.get(a..b).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0);
    d * 86400.0 + n(11, 13) * 3600.0 + n(14, 16) * 60.0 + n(17, 19)
}

const PHASE_GAP: f64 = 600.0;
const RAID_GAP: f64 = 90.0;
const MAX_PHASES: usize = 6;

fn close(a: i64, b: i64) -> bool {
    (a - b).abs() * 50 <= a.max(b)
}

const RAIDS: &[(&str, &[i64])] = &[
    ("拉圖斯", &[53_750_000, 12_450_310]),
    ("拉圖斯（困難）", &[160_000_000, 20_000_000]),
    ("殘暴炎魔", &[66_000_000, 68_000_000, 70_000_000]),
    ("闇黑龍王", &[134_860_000, 134_860_000, 1_848_000_000]),
    ("克雷塞爾", &[315_000_000, 472_500_000]),
    ("艾畢奈亞", &[260_000_000, 260_000_000]),
];

fn raid_phases(max: i64) -> Option<(usize, Vec<usize>)> {
    RAIDS.iter().enumerate().find_map(|(i, (_, hps))| {
        let ph: Vec<usize> = (0..hps.len()).filter(|&k| (max - hps[k]).abs() * 200 <= hps[k] * 3).collect();
        (!ph.is_empty()).then_some((i, ph))
    })
}

pub fn raid_name(max: i64) -> Option<&'static str> {
    raid_phases(max).map(|(i, _)| RAIDS[i].0)
}

fn alike(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a == b {
        return true;
    }
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            d[i][j] = if a[i - 1] == b[j - 1] { d[i - 1][j - 1] + 1 } else { d[i - 1][j].max(d[i][j - 1]) };
        }
    }
    let l = d[a.len()][b.len()];
    l >= 2 && l * 3 >= a.len().min(b.len()) * 2
}

fn boss_names(recs: &[BossRec]) -> Vec<String> {
    let mut names: Vec<(&str, usize)> = vec![];
    for r in recs {
        match names.iter_mut().find(|n| n.0 == r.name) {
            Some(n) => n.1 += 1,
            None => names.push((&r.name, 1)),
        }
    }
    let mut parent: Vec<usize> = (0..names.len()).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            i = p[i];
        }
        i
    }
    let idx = |name: &str| names.iter().position(|n| n.0 == name).unwrap();
    for x in recs {
        for y in recs {
            if x.name != y.name && close(x.max, y.max) && alike(&x.name, &y.name) {
                let (a, b) = (root(&mut parent, idx(&x.name)), root(&mut parent, idx(&y.name)));
                parent[a.max(b)] = a.min(b);
            }
        }
    }
    let mut best: Vec<usize> = (0..names.len()).collect();
    for i in 0..names.len() {
        let r = root(&mut parent, i);
        if names[i].1 > names[best[r]].1 {
            best[r] = i;
        }
    }
    recs.iter().map(|r| raid_name(r.max).unwrap_or(names[best[root(&mut parent, idx(&r.name))]].0).to_string()).collect()
}

fn group_boss(recs: &[BossRec]) -> Vec<Vec<usize>> {
    let n = recs.len();
    let names = boss_names(recs);
    let mut kinds: Vec<(&str, i64)> = vec![];
    let id: Vec<usize> = recs
        .iter()
        .zip(&names)
        .map(|(r, name)| match kinds.iter().position(|k| k.0 == name && close(k.1, r.max)) {
            Some(i) => i,
            None => {
                kinds.push((name, r.max));
                kinds.len() - 1
            }
        })
        .collect();
    let mut end = vec![n; n];
    for i in (0..n.saturating_sub(1)).rev() {
        end[i] = if recs[i + 1].start - (recs[i].start + recs[i].secs) <= PHASE_GAP { end[i + 1] } else { i + 1 };
    }
    let raid: Vec<Option<(usize, Vec<usize>)>> = recs.iter().map(|r| raid_phases(r.max)).collect();
    let plain = |j: usize, len: usize| raid[j..j + len].iter().all(|x| x.is_none());
    let periodic = |w: &[usize]| (1..w.len()).any(|p| (0..w.len() - p).all(|j| w[j] == w[j + p]));
    let one_name = |j: usize, len: usize| names[j..j + len].iter().all(|x| *x == names[j]);
    let mut count: HashMap<&[usize], usize> = HashMap::new();
    let mut lead: HashMap<&[usize], usize> = HashMap::new();
    let mut gaps: HashMap<&[usize], f64> = HashMap::new();
    let gap = |k: usize| recs[k + 1].start - (recs[k].start + recs[k].secs);
    let mut a = 0;
    while a < n {
        let b = end[a];
        for len in 2..=MAX_PHASES.min(b - a) {
            let mut taken: HashMap<&[usize], usize> = HashMap::new();
            for j in a..=b - len {
                let w = &id[j..j + len];
                if periodic(w) || !one_name(j, len) || !plain(j, len) || taken.get(w).map_or(false, |&e| j < e) {
                    continue;
                }
                taken.insert(w, j + len);
                *count.entry(w).or_insert(0) += 1;
                *gaps.entry(w).or_insert(0.0) += (j..j + len - 1).map(gap).sum::<f64>();
                if j == a {
                    *lead.entry(w).or_insert(0) += 1;
                }
            }
        }
        a = b;
    }
    let score = |w: &[usize]| (lead.get(w).copied().unwrap_or(0), gaps.get(w).copied().unwrap_or(0.0) / count.get(w).copied().unwrap_or(1) as f64);
    let ok = |w: &[usize]| {
        let (l, g) = score(w);
        count.get(w).map_or(false, |&c| c >= 2)
            && l > 0
            && (1..w.len()).all(|r| {
                let rot: Vec<usize> = w[r..].iter().chain(&w[..r]).copied().collect();
                let (l2, g2) = score(&rot);
                !(l2 > l || (l2 == l && g2 < g))
            })
    };
    let mut out = vec![];
    let mut i = 0;
    while i < n {
        if let Some((ri, ph)) = &raid[i] {
            let (mut p, mut j) = (ph[0], i + 1);
            while j < end[i] {
                let Some((rj, pj)) = &raid[j] else { break };
                let Some(&q) = pj.iter().find(|&&q| q > p).filter(|_| rj == ri) else { break };
                if pj.contains(&p) && recs[j].start - (recs[j - 1].start + recs[j - 1].secs) > RAID_GAP {
                    break;
                }
                p = q;
                j += 1;
            }
            out.push((i..j).collect());
            i = j;
            continue;
        }
        let len = (2..=MAX_PHASES.min(end[i] - i)).rev().find(|&l| plain(i, l) && ok(&id[i..i + l])).unwrap_or(1);
        out.push((i..i + len).collect());
        i += len;
    }
    out
}

struct BossRun<'a> {
    parts: Vec<&'a BossRec>,
    name: &'a str,
}

impl BossRun<'_> {
    fn first(&self) -> &BossRec {
        self.parts[0]
    }
    fn last(&self) -> &BossRec {
        self.parts[self.parts.len() - 1]
    }
    fn secs(&self) -> f64 {
        self.last().start + self.last().secs - self.first().start
    }
    fn dpm(&self) -> f64 {
        if self.parts.len() == 1 {
            return self.first().dpm;
        }
        let secs: f64 = self.parts.iter().map(|p| p.secs).sum();
        let dmg: i64 = self.parts.iter().map(|p| p.dmg).sum();
        if secs > 0.0 { dmg as f64 / secs * 60.0 } else { 0.0 }
    }
    fn key(&self) -> String {
        format!("{} {}", rec_key(self.first()), self.parts.len())
    }
    fn curve(&self, curves: &HashMap<String, Vec<(f64, i64)>>) -> Option<Vec<(f64, i64)>> {
        let mut out = vec![];
        let mut dd = 0;
        for p in &self.parts {
            let c = curves.get(&curve_key(&p.when, &p.name, p.secs))?;
            let dt = p.start - self.first().start;
            out.extend(c.iter().map(|&(t, d)| (t + dt, d + dd)));
            dd += p.dmg;
        }
        Some(out)
    }
}

pub fn load_boss(path: &std::path::Path) -> Vec<BossRec> {
    let Ok(raw) = std::fs::read(path) else { return vec![] };
    let text = String::from_utf8_lossy(raw.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&raw)).to_string();
    let mut v: Vec<BossRec> = text.lines().filter_map(parse_boss).collect();
    v.sort_by(|a, b| a.when.cmp(&b.when));
    v
}

fn curve_key(when: &str, name: &str, secs: f64) -> String {
    format!("{when} {name} {}", secs.round() as i64)
}

pub fn load_boss_curves(path: &Path) -> HashMap<String, Vec<(f64, i64)>> {
    let mut m = HashMap::new();
    let Ok(text) = std::fs::read_to_string(path) else { return m };
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let (Some(when), Some(name), Some(secs), Some(pts)) = (v["when"].as_str(), v["name"].as_str(), v["secs"].as_f64(), v["pts"].as_array()) else { continue };
        let pts: Vec<(f64, i64)> = pts.iter().filter_map(|p| Some((p.get(0)?.as_f64()?, p.get(1)?.as_i64()?))).collect();
        m.insert(curve_key(when, name, secs), pts);
    }
    m
}

fn days_of(day: &str) -> Option<i64> {
    let y: i64 = day.get(0..4)?.parse().ok()?;
    let m: i64 = day.get(5..7)?.parse().ok()?;
    let d: i64 = day.get(8..10)?.parse().ok()?;
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe - 719468)
}

fn weekday(day: &str) -> &'static str {
    const W: [&str; 7] = ["四", "五", "六", "日", "一", "二", "三"];
    days_of(day).map_or("", |d| W[d.rem_euclid(7) as usize])
}

fn md(day: &str) -> String {
    format!("{}/{}", day.get(5..7).unwrap_or(""), day.get(8..10).unwrap_or(""))
}

fn hm_len(min: f64) -> String {
    let m = min.round() as i64;
    if m >= 60 { format!("{}小時 {}分", m / 60, m % 60) } else { format!("{m}分") }
}

fn mmss(sec: f64) -> String {
    let s = sec.max(0.0).round() as i64;
    format!("{}:{:02}", s / 60, s % 60)
}

fn del_button(ui: &mut egui::Ui) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(24.0, 22.0), Sense::click());
    let hot = resp.hovered();
    if hot {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(r, WIDGET_RADIUS, BG);
    }
    let c = r.center();
    let st = Stroke::new(1.6_f32, if hot { HP } else { MUTED });
    ui.painter().line_segment([c + vec2(-4.5, -4.5), c + vec2(4.5, 4.5)], st);
    ui.painter().line_segment([c + vec2(4.5, -4.5), c + vec2(-4.5, 4.5)], st);
    resp.on_hover_text("刪除這一筆").clicked()
}

pub(crate) struct Undo {
    pub(crate) label: String,
    pub(crate) lines: Vec<(PathBuf, Vec<String>)>,
}

enum Act {
    Delete(Seg),
    Undo,
    BossDelete(Vec<BossRec>),
    BossUndo,
}

#[derive(Default)]
pub(crate) struct BossTab {
    recs: Vec<BossRec>,
    groups: Vec<Vec<usize>>,
    names: Vec<String>,
    curves: HashMap<String, Vec<(f64, i64)>>,
    pub(crate) open: Option<String>,
    pub(crate) confirm: Option<String>,
    pub(crate) undo: Option<Undo>,
}

fn rec_key(r: &BossRec) -> String {
    format!("{} {} {} {}", r.when, r.name, r.secs, r.dpm)
}

fn in_range(range: usize, day: &str, tday: i64) -> bool {
    let d = days_of(day).unwrap_or(0);
    match range {
        0 => d == tday,
        1 => d > tday - 7,
        2 => d > tday - 30,
        _ => true,
    }
}

#[derive(Default, Clone, Copy)]
struct Sum {
    n: usize,
    min: f64,
    exp: i64,
    cost: f64,
    meso: i64,
    sold: i64,
    net: f64,
    used: [i64; 2],
    levelups: i64,
}

fn sum(list: &[&Seg]) -> Sum {
    let mut s = Sum::default();
    for x in list {
        s.n += 1;
        s.min += x.min;
        s.exp += x.exp;
        s.cost += x.cost;
        s.meso += x.meso.unwrap_or(0);
        s.sold += x.sold;
        s.net += x.net.unwrap_or(-x.cost);
        s.used[0] += x.used[0];
        s.used[1] += x.used[1];
        s.levelups += x.levelups;
    }
    s
}

const RANGES: [&str; 4] = ["今天", "近 7 天", "近 30 天", "全部"];
const METRICS: [&str; 2] = ["經驗", "收益"];
const CURVES: [&str; 3] = ["經驗", "收益", "藥水"];

pub struct History {
    segs: Vec<Seg>,
    loaded: f64,
    pub(crate) range: usize,
    pub(crate) metric: usize,
    closed: HashSet<String>,
    curves: HashMap<String, Vec<CurvePt>>,
    pub(crate) open_seg: Option<String>,
    pub(crate) curve_metric: usize,
    pub(crate) curve_wave: bool,
    scroll_to: bool,
    pub(crate) confirm_del: Option<String>,
    pub(crate) undo: Option<Undo>,
    pub(crate) note: Option<String>,
    pub(crate) boss_tab: bool,
    pub(crate) boss: BossTab,
    pub focus: bool,
    pub prev_fg: Option<isize>,
    pub pos: Option<(i32, i32)>,
    pub size: egui::Vec2,
    pub close_req: bool,
    zoom_seen: f32,
}

impl History {
    pub fn new(prev_fg: Option<isize>) -> History {
        History {
            segs: vec![],
            loaded: f64::MIN,
            range: 1,
            metric: 0,
            closed: HashSet::new(),
            curves: HashMap::new(),
            open_seg: None,
            curve_metric: 0,
            curve_wave: true,
            scroll_to: false,
            confirm_del: None,
            undo: None,
            note: None,
            boss_tab: false,
            boss: BossTab::default(),
            focus: true,
            prev_fg,
            pos: None,
            size: vec2(600.0, 640.0),
            close_req: false,
            zoom_seen: 1.0,
        }
    }
}

pub fn window(o: &mut Overlay, ctx: &egui::Context, hwnd: isize) {
    if let Some(mut h) = o.record.take() {
        if history_ui(o, &mut h, ctx, hwnd) {
            o.record = Some(h);
        } else {
            o.last_record_fg = h.prev_fg;
        }
    }
}

fn live_seg(o: &mut Overlay) -> Option<Seg> {
    let t = &mut o.t;
    let a = t.active();
    if a < 60.0 {
        return None;
    }
    let s = t.stats();
    let start = t.started_at.clone().unwrap_or_else(tracker::local_iso);
    Some(Seg {
        start: start.get(..16).unwrap_or(&start).replace('T', " "),
        min: a / 60.0,
        exp: t.gain,
        levelups: t.levelups,
        used: [t.used(0), t.used(1)],
        cost: s.cost,
        meso: s.income,
        sold: s.sold,
        names: [t.pot_label(0), t.pot_label(1)],
        extra: t.extra_label(),
        net: s.profit,
        live: true,
    })
}

const COLS: [f32; 5] = [86.0, 74.0, 104.0, 120.0, 136.0];

fn wan(v: f64) -> String {
    if v.abs() >= 10_000.0 || v == 0.0 { short(Some(v)) } else { format!("{:.2}萬", v / 10_000.0) }
}

fn sign_col(v: f64) -> Color32 {
    if v >= 0.0 { GOOD } else { HP }
}

fn cell(ui: &mut egui::Ui, w: f32, right: bool, top: RichText, sub: Option<egui::WidgetText>) {
    let layout = Layout::top_down(if right { Align::Max } else { Align::Min });
    ui.allocate_ui_with_layout(vec2(w, 0.0), layout, |ui| {
        ui.set_width(w);
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add(egui::Label::new(top).selectable(false));
        if let Some(s) = sub {
            ui.add(egui::Label::new(s).selectable(false));
        }
    });
}

fn chip(ui: &mut egui::Ui, text: &str, on: bool, size: f32) -> bool {
    let (fill, fg) = if on { (MP, Color32::from_rgb(0x0b, 0x14, 0x26)) } else { (PANEL, MUTED) };
    let b = egui::Button::new(RichText::new(text).font(if on { bold(size) } else { font(size) }).color(fg)).fill(fill).corner_radius(WIDGET_RADIUS).stroke(Stroke::NONE);
    let r = ui.add(b);
    if r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    r.clicked()
}

fn card(ui: &mut egui::Ui, w: f32, title: &str, big: RichText, sub: impl Into<egui::WidgetText>, sub2: Option<egui::WidgetText>) {
    egui::Frame::new().fill(PANEL).corner_radius(8).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.set_width(w - 20.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            let rh = |f: egui::FontId| ui.fonts_mut(|x| x.row_height(&f));
            let lines = if sub2.is_some() { 2.0 } else { 1.0 };
            let hh = rh(font(8.5)) * (1.0 + lines) + rh(bold(13.0)).max(rh(mono(13.0, true))) + 2.0 * (1.0 + lines) + 6.0;
            ui.set_min_height(hh);
            ui.add(egui::Label::new(RichText::new(title).font(font(8.5)).color(MUTED)).selectable(false));
            ui.add(egui::Label::new(big).selectable(false).truncate());
            ui.add(egui::Label::new(sub).selectable(false).truncate());
            if let Some(s) = sub2 {
                ui.add(egui::Label::new(s).selectable(false).truncate());
            }
        });
    });
}

fn nice(lo0: f64, hi0: f64) -> (f64, f64, f64) {
    let raw = (hi0 - lo0) / 3.0;
    let mag = 10f64.powf(raw.log10().floor());
    let step = mag * [1.0, 2.0, 5.0, 10.0].into_iter().find(|&k| raw <= k * mag).unwrap_or(10.0);
    ((lo0 / step).floor() * step, (hi0 / step).ceil() * step, step)
}

fn curve_chart(ui: &mut egui::Ui, w: f32, xs: &[f64], lines: &[(Vec<f64>, Color32)], fmt: &dyn Fn(f64) -> String, tip: &dyn Fn(usize) -> String) {
    curve_chart_x(ui, w, xs, lines, fmt, tip, false);
}

fn curve_chart_x(ui: &mut egui::Ui, w: f32, xs: &[f64], lines: &[(Vec<f64>, Color32)], fmt: &dyn Fn(f64) -> String, tip: &dyn Fn(usize) -> String, clock: bool) {
    let (resp, p) = ui.allocate_painter(vec2(w, if xs.len() < 2 { 56.0 } else { 150.0 }), Sense::hover());
    let r = resp.rect;
    p.rect_filled(r, 6.0, BG);
    if xs.len() < 2 {
        p.text(r.center(), Align2::CENTER_CENTER, "無紀錄（需更新v1.5後才能產生新紀錄）", font(9.0), MUTED);
        return;
    }
    let plot = Rect::from_min_max(pos2(r.left() + 52.0, r.top() + 14.0), pos2(r.right() - 16.0, r.bottom() - 24.0));
    let all = lines.iter().flat_map(|l| l.0.iter().cloned());
    let lo0 = all.clone().fold(0.0, f64::min);
    let hi0 = all.fold(0.0, f64::max).max(lo0 + 3.0);
    let (lo, hi, step) = nice(lo0, hi0);
    let xmax = xs.last().cloned().unwrap_or(1.0).max(1.0);
    let y_of = |v: f64| plot.bottom() - ((v - lo) / (hi - lo)) as f32 * plot.height();
    let x_of = |m: f64| plot.left() + (m / xmax) as f32 * plot.width();
    let grid = Stroke::new(1.0_f32, Color32::from_rgb(0x2a, 0x33, 0x44));
    for k in 0..=((hi - lo) / step).round() as i64 {
        let v = lo + step * k as f64;
        p.line_segment([pos2(plot.left(), y_of(v)), pos2(plot.right(), y_of(v))], grid);
        p.text(pos2(plot.left() - 8.0, y_of(v)), Align2::RIGHT_CENTER, fmt(v).replace(".0萬", "萬"), mono(7.5, false), MUTED);
    }
    if lo < 0.0 {
        p.line_segment([pos2(plot.left(), y_of(0.0)), pos2(plot.right(), y_of(0.0))], Stroke::new(1.0_f32, MUTED.gamma_multiply(0.6)));
    }
    let steps: &[f64] = if clock { &[0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0] } else { &[1.0, 2.0, 5.0, 10.0, 15.0, 20.0, 30.0, 60.0, 120.0, 180.0, 360.0] };
    let xstep = steps.iter().copied().find(|&s| xmax / s <= 6.0).unwrap_or(600.0);
    let mut m = 0.0;
    while m <= xmax + 1e-6 {
        let lab = if clock { mmss(m * 60.0) } else if xstep >= 60.0 { format!("{}小時", m / 60.0) } else { format!("{m}分") };
        p.text(pos2(x_of(m), plot.bottom() + 6.0), Align2::CENTER_TOP, lab, mono(7.5, false), MUTED);
        m += xstep;
    }
    let pts: Vec<Vec<Pos2>> = lines.iter().map(|l| xs.iter().zip(&l.0).map(|(&x, &v)| pos2(x_of(x), y_of(v))).collect()).collect();
    if let [(_, col)] = lines {
        let fill = Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 30);
        let base = y_of(0.0_f64.clamp(lo, hi));
        for w2 in pts[0].windows(2) {
            let (a, b) = (w2[0], w2[1]);
            p.add(Shape::convex_polygon(vec![a, b, pos2(b.x, base), pos2(a.x, base)], fill, Stroke::NONE));
        }
    }
    for (l, ps) in lines.iter().zip(&pts) {
        p.add(Shape::line(ps.clone(), Stroke::new(2.0_f32, l.1)));
    }
    let hover = resp.hover_pos().filter(|h| plot.expand2(vec2(8.0, 14.0)).contains(*h)).map(|h| {
        (0..xs.len()).min_by(|&a, &b| (x_of(xs[a]) - h.x).abs().total_cmp(&(x_of(xs[b]) - h.x).abs())).unwrap()
    });
    if let Some(i) = hover {
        let x = x_of(xs[i]);
        p.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], Stroke::new(1.0_f32, MUTED.gamma_multiply(0.5)));
        for (l, ps) in lines.iter().zip(&pts) {
            p.circle_filled(ps[i], 4.0, l.1);
            p.circle_stroke(ps[i], 4.0, Stroke::new(1.5_f32, BG));
        }
        let g = p.layout_no_wrap(tip(i), mono(8.5, true), INK);
        let mut bx = Rect::from_min_size(pos2(x + 10.0, plot.top() - 6.0), g.size() + vec2(14.0, 8.0));
        if bx.right() > r.right() - 4.0 {
            bx = bx.translate(vec2(-bx.width() - 20.0, 0.0));
        }
        p.rect_filled(bx, 5.0, PANEL);
        p.rect_stroke(bx, 5.0, Stroke::new(1.0_f32, LINE), egui::StrokeKind::Inside);
        p.galley(bx.min + vec2(7.0, 4.0), g, INK);
    }
}

fn buckets(c: &[CurvePt]) -> (f64, Vec<(f64, f64, usize, usize)>) {
    let len = (c.last().map_or(0.0, |p| p.0) - c[0].0) / 60.0;
    let b = [0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0].into_iter().find(|&m| len / m <= 40.0).unwrap_or(60.0);
    let mut out = vec![];
    let mut i0 = 0;
    for i in 1..c.len() {
        if c[i].0 - c[i0].0 >= b * 60.0 - 1.0 {
            out.push((c[i0].0 / 60.0, c[i].0 / 60.0, i0, i));
            i0 = i;
        }
    }
    let last = c.len() - 1;
    if i0 < last && c[last].0 - c[i0].0 >= b * 30.0 {
        out.push((c[i0].0 / 60.0, c[last].0 / 60.0, i0, last));
    }
    (b, out)
}

fn seg_detail(ui: &mut egui::Ui, w: f32, x: &Seg, curve: Option<&[CurvePt]>, metric: &mut usize, wave: &mut bool) {
    ui.spacing_mut().item_spacing.y = 6.0;
    let net = x.net.unwrap_or(-x.cost);
    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let cw = w / if x.sold > 0 { 5.0 } else { 4.0 };
        let stat = |ui: &mut egui::Ui, title: &str, v: RichText| {
            ui.allocate_ui_with_layout(vec2(cw, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(cw);
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.add(egui::Label::new(RichText::new(title).font(font(8.0)).color(MUTED)).selectable(false));
                ui.add(egui::Label::new(v).selectable(false));
            });
        };
        stat(ui, "升級", RichText::new(format!("{}", x.levelups)).font(mono(10.0, true)).color(INK));
        stat(ui, "藥水花費", RichText::new(format!("-{}", short(Some(x.cost)))).font(mono(10.0, true)).color(HP));
        let meso = match x.meso {
            Some(m) => RichText::new(short(Some(m as f64))).font(mono(10.0, true)).color(GOOD),
            None => RichText::new("沒讀到").font(font(9.5)).color(MUTED),
        };
        stat(ui, "撿到楓幣", meso);
        if x.sold > 0 {
            stat(ui, "賣掉落物", RichText::new(short(Some(x.sold as f64))).font(mono(10.0, true)).color(GOOD));
        }
        stat(ui, "收益", RichText::new(wan(net)).font(mono(10.0, true)).color(sign_col(net)));
    });
    let c = curve.unwrap_or(&[]);
    if c.len() < 2 {
        curve_chart(ui, w, &[], &[], &|v| short(Some(v)), &|_| String::new());
        return;
    }
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(RichText::new("過程").font(bold(9.5)).color(INK)).selectable(false));
        ui.add_space(6.0);
        ui.spacing_mut().item_spacing.x = 4.0;
        for (i, n) in CURVES.iter().enumerate() {
            if chip(ui, n, *metric == i, 8.5) {
                *metric = i;
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            for (n, v) in [("分段", true), ("累積", false)] {
                if chip(ui, n, *wave == v, 8.5) {
                    *wave = v;
                }
            }
        });
    });
    let xs: Vec<f64> = c.iter().map(|p| p.0 / 60.0).collect();
    let net_of = |p: &CurvePt| p.4.unwrap_or(0) as f64 - p.5;
    let first_inc = c.iter().position(|p| p.4.is_some()).unwrap_or(0);
    let (c, xs) = if *metric == 1 { (&c[first_inc..], xs[first_inc..].to_vec()) } else { (c, xs) };
    let mins = |i: usize| format!("{}分", xs[i].round() as i64);
    if *wave {
        let (b, bs) = buckets(c);
        let per = |from: f64, to: f64| b / (to - from).max(1e-6);
        let bx: Vec<f64> = bs.iter().map(|q| q.1).collect();
        let amt = |f: &dyn Fn(&CurvePt) -> f64| -> Vec<f64> { bs.iter().map(|q| (f(&c[q.3]) - f(&c[q.2])) * per(q.0, q.1)).collect() };
        let span = |i: usize| format!("{}–{}分", bs[i].0.round() as i64, bs[i].1.round() as i64);
        match *metric {
            0 => {
                let v = amt(&|p| p.1 as f64);
                curve_chart(ui, w, &bx, &[(v.clone(), EXP)], &|v| short(Some(v)), &|i| format!("{}  經驗 +{}", span(i), short(Some(v[i]))));
            }
            1 => {
                let (v, m, k) = (amt(&net_of), amt(&|p| p.4.unwrap_or(0) as f64), amt(&|p| p.5));
                curve_chart(ui, w, &bx, &[(v.clone(), GOOD)], &|v| short(Some(v)), &|i| format!("{}  收益 {}  撿到 {}  藥水 -{}", span(i), wan(v[i]), short(Some(m[i])), short(Some(k[i]))));
            }
            _ => {
                let (r, bl) = (amt(&|p| p.2 as f64), amt(&|p| p.3 as f64));
                let n = |k: usize| if x.names[k].is_empty() { ["紅", "藍"][k].to_string() } else { x.names[k].clone() };
                curve_chart(ui, w, &bx, &[(r.clone(), HP), (bl.clone(), MP)], &|v| format!("{}", v.round() as i64), &|i| {
                    format!("{}  {} {} 瓶 · {} {} 瓶", span(i), n(0), r[i].round() as i64, n(1), bl[i].round() as i64)
                });
            }
        }
        return;
    }
    match *metric {
        0 => curve_chart(ui, w, &xs, &[(c.iter().map(|p| p.1 as f64).collect(), EXP)], &|v| short(Some(v)), &|i| format!("{}  經驗 {}", mins(i), short(Some(c[i].1 as f64)))),
        1 => curve_chart(ui, w, &xs, &[(c.iter().map(net_of).collect(), GOOD)], &|v| short(Some(v)), &|i| {
            format!("{}  收益 {}  撿到 {}  藥水 -{}", mins(i), wan(net_of(&c[i])), short(c[i].4.map(|m| m as f64)), short(Some(c[i].5)))
        }),
        _ => curve_chart(
            ui,
            w,
            &xs,
            &[(c.iter().map(|p| p.2 as f64).collect(), HP), (c.iter().map(|p| p.3 as f64).collect(), MP)],
            &|v| format!("{}", v.round() as i64),
            &|i| {
                let n = |k: usize| if x.names[k].is_empty() { ["紅", "藍"][k].to_string() } else { x.names[k].clone() };
                format!("{}  {} {} 瓶 · {} {} 瓶", mins(i), n(0), c[i].2, n(1), c[i].3)
            },
        ),
    }
}

fn line_chart(ui: &mut egui::Ui, w: f32, points: &[(String, f64, bool)], col: Color32, signed: bool, fmt: &dyn Fn(f64) -> String) {
    let (resp, p) = ui.allocate_painter(vec2(w, 176.0), Sense::hover());
    let r = resp.rect;
    p.rect_filled(r, 8.0, PANEL);
    if points.is_empty() {
        p.text(r.center(), Align2::CENTER_CENTER, "這段期間沒有紀錄", font(9.0), MUTED);
        return;
    }
    let plot = Rect::from_min_max(pos2(r.left() + 52.0, r.top() + 22.0), pos2(r.right() - 18.0, r.bottom() - 26.0));
    let vals: Vec<f64> = points.iter().map(|x| x.1).collect();
    let lo0 = vals.iter().cloned().fold(0.0, f64::min);
    let hi0 = vals.iter().cloned().fold(0.0, f64::max).max(lo0 + 1.0);
    let (lo, hi, step) = nice(lo0, hi0);
    let y_of = |v: f64| plot.bottom() - ((v - lo) / (hi - lo)) as f32 * plot.height();
    let n = points.len();
    let (x0, x1) = (plot.left() + 24.0, plot.right() - 24.0);
    let x_of = |i: usize| if n == 1 { plot.center().x } else { x0 + (x1 - x0) * i as f32 / (n - 1) as f32 };
    let ticks = ((hi - lo) / step).round() as i64;
    for k in 0..=ticks {
        let v = lo + step * k as f64;
        let y = y_of(v);
        p.line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], Stroke::new(1.0_f32, Color32::from_rgb(0x2a, 0x33, 0x44)));
        p.text(pos2(plot.left() - 8.0, y), Align2::RIGHT_CENTER, fmt(v).replace(".0萬", "萬"), mono(7.5, false), MUTED);
    }
    if lo < 0.0 {
        let y = y_of(0.0);
        p.line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], Stroke::new(1.0_f32, MUTED.gamma_multiply(0.6)));
    }
    let pts: Vec<Pos2> = (0..n).map(|i| pos2(x_of(i), y_of(vals[i]))).collect();
    let fill = Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 34);
    for w2 in pts.windows(2) {
        let (a, b) = (w2[0], w2[1]);
        p.add(Shape::convex_polygon(vec![a, b, pos2(b.x, plot.bottom()), pos2(a.x, plot.bottom())], fill, Stroke::NONE));
    }
    if n > 1 {
        p.add(Shape::line(pts.clone(), Stroke::new(2.2_f32, col)));
    }
    let every = (n as f32 / 8.0).ceil().max(1.0) as usize;
    for (i, (lab, _, _)) in points.iter().enumerate() {
        if i % every == 0 || i == n - 1 {
            p.text(pos2(x_of(i), plot.bottom() + 8.0), Align2::CENTER_TOP, lab, mono(7.5, false), MUTED);
        }
    }
    let hover = resp.hover_pos().filter(|h| r.contains(*h)).map(|h| {
        (0..n).min_by(|&a, &b| (pts[a].x - h.x).abs().total_cmp(&(pts[b].x - h.x).abs())).unwrap()
    });
    for (i, pt) in pts.iter().enumerate() {
        let c = if signed { sign_col(vals[i]) } else { col };
        let big = points[i].2 || hover == Some(i);
        p.circle_filled(*pt, if big { 4.5 } else { 3.2 }, c);
        p.circle_stroke(*pt, if big { 4.5 } else { 3.2 }, Stroke::new(1.5_f32, PANEL));
        if n <= 10 && hover.is_none() {
            let above = !signed || vals[i] >= 0.0;
            let at = pos2(pt.x, if above { pt.y - 8.0 } else { pt.y + 8.0 });
            p.text(at, if above { Align2::CENTER_BOTTOM } else { Align2::CENTER_TOP }, fmt(vals[i]), mono(7.5, true), INK);
        }
    }
    if let Some(i) = hover {
        let x = pts[i].x;
        p.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], Stroke::new(1.0_f32, MUTED.gamma_multiply(0.5)));
        let txt = format!("{}  {}", points[i].0, fmt(vals[i]));
        let g = p.layout_no_wrap(txt, mono(8.5, true), INK);
        let mut bx = Rect::from_min_size(pos2(x + 10.0, plot.top() - 4.0), g.size() + vec2(14.0, 8.0));
        if bx.right() > r.right() - 4.0 {
            bx = bx.translate(vec2(-bx.width() - 20.0, 0.0));
        }
        p.rect_filled(bx, 5.0, BG);
        p.rect_stroke(bx, 5.0, Stroke::new(1.0_f32, LINE), egui::StrokeKind::Inside);
        p.galley(bx.min + vec2(7.0, 4.0), g, INK);
    }
}

fn history_ui(o: &mut Overlay, h: &mut History, ctx: &egui::Context, hwnd: isize) -> bool {
    let ctx = ctx.clone();
    let rezoom = crate::dialogs::dialog_zoom(o, &ctx, &mut h.zoom_seen);
    if h.focus {
        h.focus = false;
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        platform::force_focus(hwnd);
    }
    let close = std::mem::take(&mut h.close_req) || ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let now = o.now();
    if now - h.loaded > 5.0 {
        h.loaded = now;
        h.segs = load(&o.paths.csv);
        h.curves = load_curves(&o.paths.csv);
        h.boss.recs = load_boss(&o.paths.here.join(BOSS_FILE));
        h.boss.groups = group_boss(&h.boss.recs);
        h.boss.names = boss_names(&h.boss.recs);
        h.boss.curves = load_boss_curves(&o.paths.here.join(BOSS_CURVE_FILE));
    }
    let tabs = crate::boss::enabled(&o.t.cfg) || !h.boss.recs.is_empty();
    if !tabs {
        h.boss_tab = false;
    }
    let live = live_seg(o);
    let live_curve = live.as_ref().filter(|l| h.open_seg.as_deref() == Some(l.start.as_str())).map(|_| o.t.curve_now());
    let today = tracker::local_iso().get(..10).unwrap_or("").to_string();
    let tday = days_of(&today).unwrap_or(0);
    let mut all: Vec<&Seg> = h.segs.iter().collect();
    if let Some(l) = &live {
        all.push(l);
    }
    let pick: Vec<&Seg> = all
        .iter()
        .copied()
        .filter(|s| in_range(h.range, s.day(), tday))
        .collect();
    let tot = sum(&pick);
    let mut act: Option<Act> = None;
    let (_, _, _, sh) = platform::primary_screen();
    let list_h = (sh as f32 / ctx.pixels_per_point() * 0.85 - 430.0).clamp(150.0, 460.0);
    let resp = egui::Area::new(egui::Id::new("hist")).fixed_pos(Pos2::ZERO).constrain(false).show(&ctx, |ui| {
        egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(16, 14)).show(ui, |ui| {
            ui.style_mut().interaction.selectable_labels = false;
            let width = COLS.iter().sum::<f32>() + 40.0;
            ui.set_max_width(width);
            ui.set_min_width(width);
            ui.allocate_ui_with_layout(vec2(width, 24.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.x = 6.0;
                for (i, n) in RANGES.iter().enumerate() {
                    if chip(ui, n, h.range == i, 9.5) {
                        h.range = i;
                    }
                }
                if tabs {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if chip(ui, "打王", h.boss_tab, 9.5) {
                            h.boss_tab = true;
                        }
                        if chip(ui, "練功", !h.boss_tab, 9.5) {
                            h.boss_tab = false;
                        }
                    });
                }
            });
            ui.add_space(10.0);
            if h.boss_tab {
                boss_view(ui, &mut h.boss, h.range, tday, &h.note, width, list_h, &mut act);
                return;
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let cw = (width - 16.0) / 3.0;
                let two = tot.sold > 0;
                let blank = || two.then(|| RichText::new(" ").font(font(8.5)).into());
                card(ui, cw, "練功時間", RichText::new(hm_len(tot.min)).font(bold(13.0)).color(INK), RichText::new(format!("{} 筆", tot.n)).font(font(8.5)).color(MUTED), blank());
                let lv = if tot.levelups > 0 { format!("升 {} 等", tot.levelups) } else { " ".into() };
                card(ui, cw, "總經驗", RichText::new(short(Some(tot.exp as f64))).font(mono(13.0, true)).color(EXP), RichText::new(lv).font(font(8.5)).color(MUTED), blank());
                card(
                    ui,
                    cw,
                    "總收益",
                    RichText::new(wan(tot.net)).font(mono(13.0, true)).color(sign_col(tot.net)),
                    colored(
                        &if two {
                            vec![("撿到 ".into(), MUTED), (short(Some(tot.meso as f64)), GOOD), (" · 賣 ".into(), MUTED), (short(Some(tot.sold as f64)), GOOD)]
                        } else {
                            vec![("撿到 ".into(), MUTED), (short(Some(tot.meso as f64)), GOOD), (" · 藥水 ".into(), MUTED), (format!("-{}", short(Some(tot.cost))), HP)]
                        },
                        font(8.5),
                    ),
                    two.then(|| colored(&[("藥水 ".into(), MUTED), (format!("-{}", short(Some(tot.cost))), HP)], font(8.5)).into()),
                );
            });
            ui.add_space(14.0);
            ui.allocate_ui_with_layout(vec2(width, 22.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_width(width);
                ui.add(egui::Label::new(RichText::new(if h.range == 0 { "走勢（每一筆）" } else { "走勢（每天）" }).font(bold(10.0)).color(INK)).selectable(false));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for i in (0..METRICS.len()).rev() {
                        if chip(ui, METRICS[i], h.metric == i, 8.5) {
                            h.metric = i;
                        }
                    }
                });
            });
            ui.add_space(4.0);
            let groups: Vec<(String, Vec<&Seg>)> = if h.range == 0 {
                pick.iter().map(|s| (s.hm().to_string(), vec![*s])).collect()
            } else {
                let mut g: Vec<(String, Vec<&Seg>)> = vec![];
                for s in &pick {
                    match g.last_mut() {
                        Some((d, v)) if d == s.day() => v.push(*s),
                        _ => g.push((s.day().to_string(), vec![*s])),
                    }
                }
                g
            };
            let pts: Vec<(String, f64, bool)> = groups
                .iter()
                .map(|(k, v)| {
                    let s = sum(v);
                    let val = if h.metric == 0 { s.exp as f64 } else { s.net - s.sold as f64 };
                    let lab = if h.range == 0 { k.clone() } else { md(k) };
                    let mark = if h.range == 0 { v.iter().any(|x| x.live) } else { *k == today };
                    (lab, val, mark)
                })
                .collect();
            let col = if h.metric == 0 { EXP } else { GOOD };
            line_chart(ui, width, &pts, col, h.metric == 1, &|v| short(Some(v)));
            ui.add_space(14.0);
            ui.add(egui::Label::new(RichText::new("紀錄").font(bold(10.0)).color(INK)).selectable(false));
            ui.add_space(4.0);
            if let Some(n) = &h.note {
                ui.add(egui::Label::new(RichText::new(n).font(font(9.0)).color(HP)).selectable(false));
                ui.add_space(4.0);
            }
            if let Some(u) = &h.undo {
                egui::Frame::new().fill(PANEL).corner_radius(6).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                    ui.set_width(width - 20.0);
                    ui.allocate_ui_with_layout(vec2(width - 20.0, 22.0), Layout::left_to_right(Align::Center), |ui| {
                        ui.add(egui::Label::new(RichText::new(format!("已刪除 {} 那一筆", u.label)).font(font(9.5)).color(INK)).selectable(false));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if crate::dialogs::button(ui, "復原", MP, Color32::from_rgb(0x0b, 0x14, 0x26), 9.0, 48.0) {
                                act = Some(Act::Undo);
                            }
                        });
                    });
                });
                ui.add_space(6.0);
            }
            ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(10.0);
                let head = |t: &str| RichText::new(t).font(font(8.0)).color(MUTED);
                cell(ui, COLS[0], false, head("開始"), None);
                cell(ui, COLS[1], true, head("時間"), None);
                cell(ui, COLS[2], true, head("經驗"), None);
                cell(ui, COLS[3], true, head("藥水 紅／藍"), None);
                cell(ui, COLS[4], true, head("收益"), None);
            });
            ui.add_space(2.0);
            egui::ScrollArea::vertical().max_height(list_h).min_scrolled_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
                let mut days: Vec<(String, Vec<&Seg>)> = vec![];
                for s in &pick {
                    match days.last_mut() {
                        Some((d, v)) if d == s.day() => v.push(*s),
                        _ => days.push((s.day().to_string(), vec![*s])),
                    }
                }
                if days.is_empty() {
                    ui.add_space(10.0);
                    ui.add(egui::Label::new(RichText::new("這段期間沒有紀錄").font(font(9.0)).color(MUTED)));
                }
                for (day, v) in days.iter().rev() {
                    let s = sum(v);
                    let open = !h.closed.contains(day);
                    let fr = egui::Frame::new().fill(PANEL).corner_radius(6).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                        ui.set_width(width - 20.0);
                        ui.horizontal(|ui| {
                            let tag = if *day == today { "  今天" } else { "" };
                            ui.add(egui::Label::new(RichText::new(format!("{} {}（{}）{tag}", if open { "▾" } else { "▸" }, md(day), weekday(day))).font(bold(10.0)).color(INK)).selectable(false));
                            ui.add_space(8.0);
                            ui.add(
                                egui::Label::new(RichText::new(format!("{} 筆 · {} · 經驗 {}", s.n, hm_len(s.min), short(Some(s.exp as f64)))).font(font(8.5)).color(MUTED))
                                    .selectable(false),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add(egui::Label::new(RichText::new(wan(s.net)).font(mono(10.0, true)).color(sign_col(s.net))).selectable(false));
                            });
                        });
                    });
                    let hit = ui.interact(fr.response.rect, egui::Id::new(("day", day.as_str())), Sense::click());
                    if hit.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if hit.clicked() {
                        if open {
                            h.closed.insert(day.clone());
                        } else {
                            h.closed.remove(day);
                        }
                    }
                    if open {
                        for x in v.iter().rev() {
                            ui.add_space(6.0);
                            let is_open = h.open_seg.as_deref() == Some(x.start.as_str());
                            let bg = ui.painter().add(Shape::Noop);
                            let row = ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                ui.add_space(10.0);
                                let t = if x.live { RichText::new(format!("{} 進行中", x.hm())).font(font(9.0)).color(GOOD) } else { RichText::new(x.hm()).font(mono(9.5, false)).color(MUTED) };
                                cell(ui, COLS[0], false, t, None);
                                cell(ui, COLS[1], true, RichText::new(hm_len(x.min)).font(font(9.5)).color(INK), None);
                                cell(
                                    ui,
                                    COLS[2],
                                    true,
                                    RichText::new(short(Some(x.exp as f64))).font(mono(10.0, true)).color(EXP),
                                    None,
                                );
                                let mut job = egui::text::LayoutJob::default();
                                let tf = |c| egui::TextFormat { font_id: mono(10.0, true), color: c, ..Default::default() };
                                job.append(&comma(x.used[0]), 0.0, tf(HP));
                                job.append(" / ", 0.0, tf(MUTED));
                                job.append(&comma(x.used[1]), 0.0, tf(MP));
                                let layout = Layout::top_down(Align::Max);
                                let pot = ui
                                    .allocate_ui_with_layout(vec2(COLS[3], 0.0), layout, |ui| {
                                        ui.set_width(COLS[3]);
                                        ui.spacing_mut().item_spacing.y = 0.0;
                                        ui.add(egui::Label::new(job).selectable(false));
                                        ui.add(egui::Label::new(RichText::new(format!("-{}", short(Some(x.cost)))).font(font(7.5)).color(HP)).selectable(false));
                                    })
                                    .response
                                    .rect;
                                let net = x.net.unwrap_or(-x.cost);
                                let sub: Option<egui::WidgetText> = if x.levelups > 0 {
                                    Some(RichText::new(format!("升 {} 等", x.levelups)).font(font(7.5)).color(EXP).into())
                                } else if x.meso.is_none() {
                                    Some(RichText::new("沒讀到楓幣").font(font(7.5)).color(MUTED).into())
                                } else if x.sold > 0 {
                                    let ps = [("撿到 ".into(), MUTED), (short(x.meso.map(|m| m as f64)), GOOD), (" · 賣 ".into(), MUTED), (short(Some(x.sold as f64)), GOOD)];
                                    Some(colored(&ps, font(7.5)).into())
                                } else {
                                    Some(RichText::new(format!("撿到 {}", short(x.meso.map(|m| m as f64)))).font(font(7.5)).color(GOOD).into())
                                };
                                cell(ui, COLS[4], true, RichText::new(wan(net)).font(mono(10.0, true)).color(sign_col(net)), sub);
                                pot
                            });
                            let left = ui.max_rect().left();
                            let rr = Rect::from_min_max(pos2(left, row.response.rect.top() - 4.0), pos2(left + width, row.response.rect.bottom() + 4.0));
                            let hit = ui.interact(rr, egui::Id::new(("seg", x.start.as_str())), Sense::click());
                            if let Some(p) = hit.hover_pos().filter(|p| !x.names[0].is_empty() && row.inner.contains(*p)) {
                                let mut parts = vec![
                                    (x.names[0].clone(), HP),
                                    (format!(" {} 瓶\n", comma(x.used[0])), INK),
                                    (x.names[1].clone(), MP),
                                    (format!(" {} 瓶", comma(x.used[1])), INK),
                                ];
                                for e in x.extra.split('、').filter(|e| !e.is_empty()) {
                                    let (n, c) = e.rsplit_once(' ').unwrap_or((e, "0"));
                                    let c = c.parse::<i64>().map_or(c.to_string(), comma);
                                    parts.push((format!("\n{n}"), EXP));
                                    parts.push((format!(" {c} 瓶"), INK));
                                }
                                let tip = colored(&parts, font(9.0));
                                egui::Area::new(egui::Id::new("pot_tip")).order(egui::Order::Tooltip).fixed_pos(p + vec2(12.0, 14.0)).interactable(false).show(ui.ctx(), |ui| {
                                    egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0_f32, LINE)).corner_radius(5).inner_margin(egui::Margin::symmetric(8, 5)).show(ui, |ui| {
                                        ui.add(egui::Label::new(tip).selectable(false).extend());
                                    });
                                });
                            }
                            if hit.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if hit.hovered() || is_open {
                                let fill = if is_open { PANEL } else { Color32::from_rgb(0x1a, 0x21, 0x2d) };
                                ui.painter().set(bg, Shape::rect_filled(rr, 6.0, fill));
                            }
                            if hit.clicked() {
                                h.open_seg = if is_open { None } else { Some(x.start.clone()) };
                                h.scroll_to = !is_open;
                                h.confirm_del = None;
                            }
                            if is_open {
                                ui.add_space(4.0);
                                let curve = if x.live { live_curve.as_deref() } else { h.curves.get(&x.start).map(|c| c.as_slice()) };
                                let dw = width - 24.0;
                                let fr = egui::Frame::new().fill(PANEL).corner_radius(6).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
                                    ui.set_width(dw);
                                    seg_detail(ui, dw, x, curve, &mut h.curve_metric, &mut h.curve_wave);
                                    if !x.live {
                                        ui.add_space(6.0);
                                        if h.confirm_del.as_deref() == Some(x.start.as_str()) {
                                            egui::Frame::new().fill(Color32::from_rgb(0x3a, 0x1f, 0x24)).corner_radius(6).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
                                                ui.allocate_ui_with_layout(vec2(dw - 20.0, 24.0), Layout::left_to_right(Align::Center), |ui| {
                                                    let q = format!("確定要刪除 {} {} 這一筆嗎？", md(x.day()), x.hm());
                                                    ui.add(egui::Label::new(RichText::new(q).font(bold(9.5)).color(INK)).selectable(false));
                                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                        if crate::dialogs::button(ui, "取消", PANEL, INK, 9.0, 48.0) {
                                                            h.confirm_del = None;
                                                        }
                                                        if crate::dialogs::button(ui, "刪除", HP, Color32::from_rgb(0x26, 0x0b, 0x0b), 9.0, 48.0) {
                                                            act = Some(Act::Delete((*x).clone()));
                                                        }
                                                    });
                                                });
                                            });
                                        } else {
                                            ui.allocate_ui_with_layout(vec2(dw, 22.0), Layout::right_to_left(Align::Center), |ui| {
                                                if del_button(ui) {
                                                    h.confirm_del = Some(x.start.clone());
                                                }
                                            });
                                        }
                                    }
                                });
                                if std::mem::take(&mut h.scroll_to) {
                                    ui.scroll_to_rect(fr.response.rect, None);
                                }
                            }
                        }
                    }
                    ui.add_space(8.0);
                }
            });
        });
    });
    crate::dialogs::fit(&ctx, &mut h.size, resp.response.rect.size(), rezoom);
    if let Some(a) = act {
        run_act(o, h, a);
    }
    !close
}

#[allow(clippy::too_many_arguments)]
fn boss_view(ui: &mut egui::Ui, b: &mut BossTab, range: usize, tday: i64, note: &Option<String>, width: f32, list_h: f32, act: &mut Option<Act>) {
    let runs: Vec<BossRun> = b
        .groups
        .iter()
        .map(|g| BossRun { parts: g.iter().map(|&i| &b.recs[i]).collect(), name: b.names.get(g[g.len() - 1]).map_or("", |s| s.as_str()) })
        .filter(|r| in_range(range, r.first().when.get(..10).unwrap_or(""), tday))
        .collect();
    ui.add(egui::Label::new(RichText::new("紀錄").font(bold(10.0)).color(INK)).selectable(false));
    ui.add_space(4.0);
    if let Some(n) = note {
        ui.add(egui::Label::new(RichText::new(n).font(font(9.0)).color(HP)).selectable(false));
        ui.add_space(4.0);
    }
    if let Some(u) = &b.undo {
        egui::Frame::new().fill(PANEL).corner_radius(6).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.set_width(width - 20.0);
            ui.allocate_ui_with_layout(vec2(width - 20.0, 22.0), Layout::left_to_right(Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new(format!("已刪除 {} 那一筆", u.label)).font(font(9.5)).color(INK)).selectable(false));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if crate::dialogs::button(ui, "復原", MP, Color32::from_rgb(0x0b, 0x14, 0x26), 9.0, 48.0) {
                        *act = Some(Act::BossUndo);
                    }
                });
            });
        });
        ui.add_space(6.0);
    }
    const BC: [f32; 4] = [130.0, 200.0, 100.0, 110.0];
    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(10.0);
        let head = |t: &str| RichText::new(t).font(font(8.0)).color(MUTED);
        cell(ui, BC[0], false, head("時間"), None);
        cell(ui, BC[1], false, head("王"), None);
        cell(ui, BC[2], true, head("耗時"), None);
        cell(ui, BC[3], true, head("DPM"), None);
    });
    ui.add_space(2.0);
    egui::ScrollArea::vertical().id_salt("boss_list").max_height(list_h).min_scrolled_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
        if runs.is_empty() {
            ui.add_space(10.0);
            ui.add(egui::Label::new(RichText::new("這段期間沒有紀錄").font(font(9.0)).color(MUTED)));
        }
        for r in runs.iter().rev() {
            let key = r.key();
            let is_open = b.open.as_deref() == Some(key.as_str());
            let when = &r.first().when;
            let (day, hm) = (when.get(..10).unwrap_or(""), when.get(11..16).unwrap_or(""));
            let name = r.name;
            ui.add_space(6.0);
            let bg = ui.painter().add(Shape::Noop);
            let row = ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(10.0);
                cell(ui, BC[0], false, RichText::new(format!("{} {hm}", md(day))).font(font(9.5)).color(MUTED), None);
                cell(ui, BC[1], false, RichText::new(name).font(font(9.5)).color(INK), None);
                cell(ui, BC[2], true, RichText::new(mmss(r.secs())).font(mono(10.0, true)).color(INK), None);
                cell(ui, BC[3], true, RichText::new(short(Some(r.dpm()))).font(mono(10.0, true)).color(MP), None);
            });
            let left = ui.max_rect().left();
            let rr = Rect::from_min_max(pos2(left, row.response.rect.top() - 4.0), pos2(left + width, row.response.rect.bottom() + 4.0));
            let hit = ui.interact(rr, egui::Id::new(("boss", key.as_str())), Sense::click());
            if hit.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if hit.hovered() || is_open {
                let fill = if is_open { PANEL } else { Color32::from_rgb(0x1a, 0x21, 0x2d) };
                ui.painter().set(bg, Shape::rect_filled(rr, 6.0, fill));
            }
            if hit.clicked() {
                b.open = if is_open { None } else { Some(key.clone()) };
                b.confirm = None;
            }
            if !is_open {
                continue;
            }
            ui.add_space(4.0);
            let dw = width - 24.0;
            egui::Frame::new().fill(PANEL).corner_radius(6).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
                ui.set_width(dw);
                boss_detail(ui, dw, r.curve(&b.curves).as_deref());
                ui.add_space(6.0);
                if b.confirm.as_deref() == Some(key.as_str()) {
                    egui::Frame::new().fill(Color32::from_rgb(0x3a, 0x1f, 0x24)).corner_radius(6).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
                        ui.allocate_ui_with_layout(vec2(dw - 20.0, 24.0), Layout::left_to_right(Align::Center), |ui| {
                            let q = format!("確定要刪除 {} {hm} {name} 這一筆嗎？", md(day));
                            ui.add(egui::Label::new(RichText::new(q).font(bold(9.5)).color(INK)).selectable(false));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if crate::dialogs::button(ui, "取消", PANEL, INK, 9.0, 48.0) {
                                    b.confirm = None;
                                }
                                if crate::dialogs::button(ui, "刪除", HP, Color32::from_rgb(0x26, 0x0b, 0x0b), 9.0, 48.0) {
                                    *act = Some(Act::BossDelete(r.parts.iter().map(|p| (*p).clone()).collect()));
                                }
                            });
                        });
                    });
                } else {
                    ui.allocate_ui_with_layout(vec2(dw, 22.0), Layout::right_to_left(Align::Center), |ui| {
                        if del_button(ui) {
                            b.confirm = Some(key.clone());
                        }
                    });
                }
            });
        }
    });
}

fn boss_detail(ui: &mut egui::Ui, w: f32, curve: Option<&[(f64, i64)]>) {
    ui.add(egui::Label::new(RichText::new("DPM").font(bold(9.5)).color(INK)).selectable(false));
    ui.add_space(4.0);
    let Some(c) = curve.filter(|c| c.len() >= 3) else {
        let (r, _) = ui.allocate_exact_size(vec2(w, 56.0), Sense::hover());
        ui.painter().rect_filled(r, 6.0, BG);
        ui.painter().text(r.center(), Align2::CENTER_CENTER, "無紀錄", font(9.0), MUTED);
        return;
    };
    let xs: Vec<f64> = c[1..].iter().map(|p| p.0 / 60.0).collect();
    let v: Vec<f64> = c.windows(2).map(|p| if p[1].0 > p[0].0 { (p[1].1 - p[0].1) as f64 / (p[1].0 - p[0].0) * 60.0 } else { 0.0 }).collect();
    curve_chart_x(ui, w, &xs, &[(v.clone(), MP)], &|x| short(Some(x)), &|i| format!("{}  DPM {}", mmss(xs[i] * 60.0), short(Some(v[i]))), true);
}

const LOCKED: &str = "練功紀錄.csv 被別的程式開著（Excel？），關掉再試一次";
const BOSS_LOCKED: &str = "打王紀錄.csv 被別的程式開著（Excel？），關掉再試一次";

fn run_act(o: &mut Overlay, h: &mut History, a: Act) {
    h.note = None;
    h.loaded = f64::MIN;
    match a {
        Act::Delete(seg) => {
            let same = |l: &str| parse_row(l).map_or(false, |s| s.start == seg.start && s.exp == seg.exp && (s.min - seg.min).abs() < 0.05);
            let mut lines = vec![];
            let mut failed = false;
            for p in csv_files(&o.paths.csv) {
                match tracker::remove_csv_lines(&p, &same) {
                    Ok(v) if !v.is_empty() => lines.push((p, v)),
                    Ok(_) => {}
                    Err(_) => failed = p.exists(),
                }
            }
            h.confirm_del = None;
            if failed {
                h.note = Some(format!("刪不掉：{LOCKED}"));
            } else if lines.is_empty() {
                h.note = Some("找不到這一筆，可能檔案剛被改過".to_string());
            }
            if !lines.is_empty() {
                h.open_seg = None;
                h.undo = Some(Undo { label: format!("{} {}", md(seg.day()), seg.hm()), lines });
            }
        }
        Act::Undo => {
            let Some(u) = h.undo.take() else { return };
            let head = o.t.csv_header();
            let mut left = vec![];
            for (p, v) in u.lines {
                if tracker::restore_csv_lines(&p, &head, &v).is_err() {
                    left.push((p, v));
                }
            }
            if !left.is_empty() {
                h.note = Some(format!("復原不了：{LOCKED}"));
                h.undo = Some(Undo { label: u.label, lines: left });
            }
        }
        Act::BossDelete(recs) => {
            let path = o.paths.here.join(BOSS_FILE);
            h.boss.confirm = None;
            match tracker::remove_csv_lines(&path, &|l| parse_boss(l).map_or(false, |r| recs.contains(&r))) {
                Ok(v) if !v.is_empty() => {
                    let (first, last) = (&recs[0], &recs[recs.len() - 1]);
                    let label = format!("{} {} {}", md(first.when.get(..10).unwrap_or("")), first.when.get(11..16).unwrap_or(""), last.name);
                    h.boss.undo = Some(Undo { label, lines: vec![(path, v)] });
                }
                Ok(_) => h.note = Some("找不到這一筆，可能檔案剛被改過".to_string()),
                Err(_) => h.note = Some(format!("刪不掉：{BOSS_LOCKED}")),
            }
        }
        Act::BossUndo => {
            let Some(u) = h.boss.undo.take() else { return };
            let head: Vec<String> = BOSS_HEAD.iter().map(|s| s.to_string()).collect();
            let mut left = vec![];
            for (p, v) in u.lines {
                if tracker::restore_csv_lines(&p, &head, &v).is_err() {
                    left.push((p, v));
                }
            }
            if !left.is_empty() {
                h.note = Some(format!("復原不了：{BOSS_LOCKED}"));
                h.boss.undo = Some(Undo { label: u.label, lines: left });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_csv_row() {
        let s = parse_row("2026-10-04 20:02,4.9,1583925,19556550,0,0,38,7,239400,2955846,66162,149818,-89582,-1106059\r").unwrap();
        assert_eq!(s.start, "2026-10-04 20:02");
        assert_eq!(s.exp, 1583925);
        assert_eq!(s.used, [38, 7]);
        assert_eq!(s.meso, Some(149818));
        assert_eq!(s.net, Some(-89582.0));
        assert!(parse_row("開始時間,練功分鐘,獲得EXP").is_none());
        let s = parse_row("2026-09-29 06:47,4.4,221925,3048966,0,0,0,0,0,0,0,,,").unwrap();
        assert_eq!(s.meso, None);
        assert_eq!(s.net, None);
    }

    #[test]
    fn boss_phases_grouped() {
        let rows = [
            "2026-10-12 10:00:00,拉圖斯,53750000,100,400,53750000,134375,8062500",
            "2026-10-12 10:06:50,拉圖斯,12450310,100,100,12450310,124503,7470186",
            "2026-10-12 12:00:00,海怒斯,65562487,99,500,64906862,129814,7788823",
            "2026-10-12 12:09:00,海怒斯,65562487,99,480,64906862,135222,8113357",
            "2026-10-12 12:17:30,海怒斯,65562487,99,490,64906862,132463,7947775",
            "2026-10-12 20:00:00,拉圖斯,53750000,100,380,53750000,141447,8486842",
            "2026-10-12 20:06:40,拉圖斯,12450310,100,90,12450310,138337,8300206",
            "2026-10-12 22:00:00,殘暴炎魔,100000000,100,600,100000000,166667,10000000",
            "2026-10-12 22:10:30,拉圖斯,12450310,100,90,12450310,138337,8300206",
        ];
        let recs: Vec<BossRec> = rows.iter().filter_map(|l| parse_boss(l)).collect();
        assert_eq!(group_boss(&recs), vec![vec![0, 1], vec![2], vec![3], vec![4], vec![5, 6], vec![7], vec![8]]);
        let run = BossRun { parts: vec![&recs[0], &recs[1]], name: "拉圖斯" };
        assert_eq!(run.secs(), 510.0);
        assert_eq!(run.dpm().round(), (66_200_310.0_f64 / 500.0 * 60.0).round());
        let r = |t: &str, max: i64| parse_boss(&format!("2026-10-14 {t},拉圖斯,{max},100,100,{max},1,1")).unwrap();
        let (c, p) = (53_750_000, 12_450_310);
        let recs = vec![r("10:00:00", c), r("10:02:00", p), r("12:00:00", p), r("12:02:10", c), r("12:04:00", p), r("12:06:10", c), r("12:08:00", p)];
        assert_eq!(group_boss(&recs), vec![vec![0, 1], vec![2], vec![3, 4], vec![5, 6]]);
        let d = |t: &str, name: &str| parse_boss(&format!("2026-10-15 {t},{name},5000000,100,30,5000000,1,1")).unwrap();
        let mut recs = vec![];
        for h in ["10", "11"] {
            for (k, name) in ["蘑菇王", "藍色蘑菇王", "殭屍蘑菇王", "肥肥", "巴洛古"].iter().enumerate() {
                recs.push(d(&format!("{h}:0{k}:00"), name));
            }
        }
        assert!(group_boss(&recs).iter().all(|g| g.len() == 1));
        let h = |t: &str, max: i64| parse_boss(&format!("2026-10-13 {t},闇黑龍王,{max},100,300,{max},1,1")).unwrap();
        let recs = vec![h("21:00:00", 134_860_000), h("21:05:10", 134_860_000), h("21:13:00", 1_848_000_000), h("23:00:00", 134_860_000), h("23:05:10", 134_860_000), h("23:12:00", 1_848_000_000)];
        assert_eq!(group_boss(&recs), vec![vec![0, 1, 2], vec![3, 4, 5]]);
        let h = |t: &str, name: &str, max: i64| parse_boss(&format!("2026-10-13 {t},{name},{max},99,300,{max},1,1")).unwrap();
        let recs = vec![
            h("20:00:15", "間黑能王", 135_403_753),
            h("20:05:15", "間黑能王", 135_787_435),
            h("20:12:48", "間黑能王", 1_862_956_309),
            h("21:00:12", "黑龍王", 136_208_616),
            h("21:05:18", "黑龍王", 135_493_551),
            h("21:13:57", "間黑龍王", 1_864_239_530),
        ];
        assert_eq!(group_boss(&recs), vec![vec![0, 1, 2], vec![3, 4, 5]]);
        assert!(boss_names(&recs).iter().all(|n| n == "闇黑龍王"));
        let z = |t: &str, max: i64| parse_boss(&format!("2026-10-12 {t},残暴炎魔,{max},99,200,{max},1,1")).unwrap();
        let recs = vec![z("16:07:36", 65_982_844), z("16:10:54", 68_002_639), z("16:14:21", 70_020_003), z("18:00:00", 65_990_000), z("18:03:30", 68_010_000), z("18:07:00", 70_000_000)];
        assert_eq!(group_boss(&recs), vec![vec![0, 1, 2], vec![3, 4, 5]]);
    }

    #[test]
    fn raids_by_table() {
        let r = |t: &str, name: &str, max: i64| parse_boss(&format!("2026-10-12 {t},{name},{max},99,100,{max},1,1")).unwrap();
        let recs = vec![r("15:00:15", "拉圖斯", 160_002_243), r("15:01:58", "拉圖斯", 20_010_493), r("16:00:00", "拉圖斯", 53_750_000), r("16:01:50", "拉圖斯", 12_450_310)];
        assert_eq!(group_boss(&recs), vec![vec![0, 1], vec![2, 3]]);
        assert_eq!(boss_names(&recs), vec!["拉圖斯（困難）", "拉圖斯（困難）", "拉圖斯", "拉圖斯"]);
        let recs = vec![
            r("20:00:00", "闇黑龍王", 134_858_225),
            r("20:01:53", "闇黑龍王", 134_860_038),
            r("21:00:00", "新王", 30_000_000),
            r("21:03:00", "新王", 9_000_000),
            r("22:00:00", "新王", 30_000_000),
            r("22:03:00", "新王", 9_000_000),
        ];
        assert_eq!(group_boss(&recs), vec![vec![0, 1], vec![2, 3], vec![4, 5]]);
        let ep = |ts: &[&str]| -> Vec<BossRec> { ts.iter().map(|t| r(t, "艾畢奈亞", 260_000_000)).collect() };
        assert_eq!(group_boss(&ep(&["10:00:00", "10:01:50", "10:05:00", "10:06:55"])), vec![vec![0, 1], vec![2, 3]]);
        assert_eq!(group_boss(&ep(&["10:00:00", "10:08:00", "10:09:50"])), vec![vec![0], vec![1, 2]]);
        let recs = vec![r("20:00:00", "闇黑龍王", 134_860_000), r("20:10:00", "闇黑龍王", 134_860_000), r("20:11:50", "闇黑龍王", 134_860_000), r("20:15:00", "闇黑龍王", 1_848_000_000)];
        assert_eq!(group_boss(&recs), vec![vec![0], vec![1, 2, 3]]);
        assert_eq!(raid_name(1_848_076_476), Some("闇黑龍王"));
        assert_eq!(raid_name(30_000_000), None);
    }

    #[test]
    fn names_alike() {
        assert!(alike("闇黑龍王", "黑龍王"));
        assert!(alike("間黑能王", "黑龍王"));
        assert!(!alike("蘑菇王", "藍色蘑菇王"));
        assert!(!alike("藍色蘑菇王", "殭屍蘑菇王"));
        assert!(!alike("拉圖斯", "拉圖斯的時鐘"));
        let r = |name: &str, max: i64| parse_boss(&format!("2026-10-13 10:00:00,{name},{max},100,30,{max},1,1")).unwrap();
        assert_eq!(boss_names(&[r("紅寶王", 5_000_000), r("藍寶王", 9_000_000)]), vec!["紅寶王", "藍寶王"]);
    }

    #[test]
    fn weekdays_and_days() {
        assert_eq!(days_of("1970-01-01"), Some(0));
        assert_eq!(weekday("2026-10-05"), "一");
        assert_eq!(weekday("2026-10-04"), "日");
        assert_eq!(days_of("2026-03-01").unwrap() - days_of("2026-02-28").unwrap(), 1);
    }
}
