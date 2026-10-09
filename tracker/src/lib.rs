pub mod exp_table;
pub mod util;

use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use util::*;

pub const NP: usize = 8;
pub const KS: [&str; NP] = ["hp", "mp", "p3", "p4", "p5", "p6", "p7", "p8"];

pub fn two<T: Default>(a: T, b: T) -> [T; NP] {
    let mut r: [T; NP] = Default::default();
    r[0] = a;
    r[1] = b;
    r
}
pub const RESUME_HOURS: f64 = 2.0;

pub fn default_cfg() -> Map<String, Value> {
    let v = json!({
        "hp_name": "馴鹿奶", "hp_price": 5600, "hp_heal": 5000,
        "mp_name": "紅豆刨冰", "mp_price": 3800, "mp_heal": 2000,
        "stack_max": 3000, "ocr_sec": 3, "alpha": 0.88, "x": 40, "y": 40, "unit": 600,
        "compact": false, "auto_pause": 3,
        "exp_region": null, "hp_region": null, "mp_region": null, "inv_region": null, "meso_region": null
    });
    v.as_object().unwrap().clone()
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map_or(false, |f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

pub type Slots = (Option<i64>, i64);

pub enum Hide {
    No,
    Fill(i64, Vec<i64>),
    Hold,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drop {
    pub miss: i64,
    pub t: f64,
    pub n: i64,
}

#[derive(Default, Clone, Debug)]
pub struct OcrOut {
    pub exp: Option<String>,
    pub exp_err: Option<String>,
    pub qs: [Option<String>; NP],
    pub qs_err: [Option<String>; NP],
    pub inv: Option<[Option<Vec<Option<i64>>>; NP]>,
    pub meso: Option<String>,
    pub meso_box_only: bool,
    pub bag_id: i64,
    pub qs_sig: [Option<Vec<f32>>; NP],
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub active: f64,
    pub rate: Option<f64>,
    pub rate_recent: Option<f64>,
    pub cost: f64,
    pub cost_hr: Option<f64>,
    pub eff: Option<f64>,
    pub hp_hr: Option<f64>,
    pub mp_hr: Option<f64>,
    pub income: Option<i64>,
    pub profit: Option<f64>,
    pub income_hr: Option<f64>,
    pub profit_hr: Option<f64>,
    pub sold: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Ui {
    OpenRecord,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Saved {
    TooShort,
    Main,
    Other(String),
    Failed,
}

pub type Snap = (f64, i64, i64, i64, Option<i64>, [i64; NP]);
pub type CurvePt = (f64, i64, i64, i64, Option<i64>, f64, i64);
pub const CURVE_FILE: &str = "練功曲線.jsonl";

pub struct Tracker {
    pub cfg: Map<String, Value>,
    pub fake_now: Option<f64>,
    pub logs: Vec<String>,
    pub ui: Vec<Ui>,
    pub tpl_exists: [bool; NP],
    pub state_path: Option<std::path::PathBuf>,
    pub log_path: Option<std::path::PathBuf>,

    pub alert: Option<(String, f64)>,
    pub ocr_msg: String,
    pub ocr_seen: f64,
    pub inv_msg: String,
    pub raw: std::collections::BTreeMap<String, String>,
    pub reset_armed: f64,
    pub running: bool,
    pub paused: bool,
    pub exp_last: Option<(i64, Option<f64>)>,
    pub need: Option<f64>,
    pub pend_first: Option<(i64, Option<f64>)>,
    pub qs: [Option<i64>; NP],
    pub qs_pend: [Option<(i64, u32)>; NP],
    pub qs_t: [f64; NP],
    pub qs_seen: [f64; NP],
    pub qs_credit: [i64; NP],
    pub qs_rise: [Option<(i64, f64)>; NP],
    pub qs_fell: [bool; NP],
    pub qs_ok: [bool; NP],
    pub qs_switched: [bool; NP],
    pub qs_blind_t: [f64; NP],
    pub qs_flat: [(u32, f64); NP],
    pub qs_flat_said: [f64; NP],
    pub qs_wrong: [Option<(i64, f64)>; NP],
    pub drift_from: [(f64, i64); NP],
    pub qs_bad: [(u32, i64); NP],
    pub qs_far_t: [f64; NP],
    pub qs_far_last: [f64; NP],
    pub qs_fixing: [bool; NP],
    pub fill_from: f64,
    pub qs_all: [i64; NP],
    pub qs_add: [i64; NP],
    pub total: [Option<(i64, i64, i64, f64)>; NP],
    pub anchor_used: [i64; NP],
    pub anchor_cells: [Vec<i64>; NP],
    pub seg_base: [Option<(i64, i64)>; NP],
    pub seg_buy: [i64; NP],
    pub seg_check_t: [f64; NP],
    pub inv_pend: [Option<(i64, i64, u32)>; NP],
    pub inv_slots: [Option<i64>; NP],
    pub inv_hide: [Option<f64>; NP],
    pub both_drop: bool,
    pub hint_pot: bool,
    pub inv_cells: [Vec<i64>; NP],
    pub total_act: [f64; NP],
    pub last_rise: [Option<(i64, i64, f64, i64, i64)>; NP],
    pub cap_since: [Option<f64>; NP],
    pub meso: Option<i64>,
    pub meso_pend: Option<(i64, u32, BTreeSet<i64>)>,
    pub meso_drop: Option<(i64, i64, f64)>,
    pub meso_rej: Option<i64>,
    pub meso_cand: Option<(i64, u32, f64)>,
    pub meso_t: f64,
    pub qs_sus: [i64; NP],
    pub exp_ok_t: f64,
    pub qs_lost: (Option<f64>, bool),
    pub qs_prev_raw: [Option<i64>; NP],
    pub exp_move_t: f64,
    pub auto_paused: bool,
    pub acc: f64,
    pub run_start: f64,
    pub started_at: Option<String>,
    pub gain: i64,
    pub hist: Vec<(f64, i64)>,
    pub levelups: i64,
    pub deaths: i64,
    pub qs_used: [i64; NP],
    pub bag_used: [i64; NP],
    pub meso_anchor: Option<(i64, f64, Option<f64>)>,
    pub inc_done: Option<(i64, f64)>,
    pub seg_pending: bool,
    pub sold: i64,
    pub pause_up: Option<(i64, i64)>,
    pub sale_ask: Option<i64>,
    pub pend_dec: Option<(i64, Option<f64>)>,
    pub reject_n: u32,
    pub pre_inc: Option<(i64, usize, f64)>,
    pub rej_run: Option<(i64, i64, u32)>,
    pub series: Vec<Snap>,
    pub curve: Vec<CurvePt>,
    pub qs_sus_cnt: [i64; NP],
    pub last_drop: [Option<Drop>; NP],
    pub recon_acc: [i64; NP],
    pub recon_add: [i64; NP],
    pub seg_t: f64,
    pub cost_prev: [f64; NP],
    pub used_mark: [i64; NP],
    pub prev_price: [Option<f64>; NP],
    pub pot_names: [Vec<String>; NP],
    pub session_no: u32,
}

pub fn sys_now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

const SESSION_KEYS: [&str; 23] = [
    "gain", "hist", "levelups", "deaths", "qs_used", "bag_used", "meso_anchor", "series", "qs_sus_cnt", "last_drop", "inc_done",
    "seg_pending", "recon_acc", "recon_add", "cost_prev", "used_mark", "prev_price", "anchor_used", "curve", "sold", "pause_up",
    "sale_ask", "pot_names",
];
const KEEP_KEYS: [&str; 10] = ["need", "qs_all", "qs_add", "total", "total_act", "inv_slots", "last_rise", "meso", "meso_t", "qs_sus"];

impl Tracker {
    pub fn new(cfg: Map<String, Value>, fake_now: Option<f64>) -> Self {
        let now = fake_now.unwrap_or_else(sys_now);
        let mut t = Tracker {
            cfg,
            fake_now,
            logs: vec![],
            ui: vec![],
            tpl_exists: [false; NP],
            state_path: None,
            log_path: None,
            alert: None,
            ocr_msg: "讀取引擎載入中…".into(),
            ocr_seen: 0.0,
            inv_msg: String::new(),
            raw: Default::default(),
            reset_armed: 0.0,
            running: false,
            paused: false,
            exp_last: None,
            need: None,
            pend_first: None,
            qs: [None; NP],
            qs_pend: [None; NP],
            qs_t: [0.0; NP],
            qs_seen: [0.0; NP],
            qs_credit: [0; NP],
            qs_rise: [None; NP],
            qs_fell: [false; NP],
            qs_ok: [false; NP],
            qs_switched: [false; NP],
            qs_blind_t: [now; NP],
            qs_flat: [(0, f64::MIN); NP],
            qs_flat_said: [f64::MIN; NP],
            qs_wrong: [None; NP],
            drift_from: [(now, 0); NP],
            qs_bad: [(0, -1); NP],
            qs_far_t: [f64::MIN; NP],
            qs_far_last: [f64::MIN; NP],
            qs_fixing: [false; NP],
            fill_from: now,
            qs_all: [0; NP],
            qs_add: [0; NP],
            total: [None; NP],
            anchor_used: [0; NP],
            anchor_cells: Default::default(),
            seg_base: [None; NP],
            seg_buy: [0; NP],
            seg_check_t: [f64::MIN; NP],
            inv_pend: [None; NP],
            inv_slots: [None; NP],
            inv_hide: [None; NP],
            both_drop: false,
            hint_pot: false,
            inv_cells: Default::default(),
            total_act: [0.0; NP],
            last_rise: [None; NP],
            cap_since: [None; NP],
            meso: None,
            meso_pend: None,
            meso_drop: None,
            meso_rej: None,
            meso_cand: None,
            meso_t: 0.0,
            qs_sus: [0; NP],
            exp_ok_t: 0.0,
            qs_lost: (None, false),
            qs_prev_raw: [None; NP],
            exp_move_t: now,
            auto_paused: false,
            acc: 0.0,
            run_start: 0.0,
            started_at: None,
            gain: 0,
            hist: vec![],
            levelups: 0,
            deaths: 0,
            qs_used: [0; NP],
            bag_used: [0; NP],
            meso_anchor: None,
            inc_done: None,
            sold: 0,
            pause_up: None,
            sale_ask: None,
            seg_pending: false,
            pend_dec: None,
            reject_n: 0,
            pre_inc: None,
            rej_run: None,
            series: vec![],
            curve: vec![],
            qs_sus_cnt: [0; NP],
            last_drop: Default::default(),
            recon_acc: [0; NP],
            recon_add: [0; NP],
            seg_t: now,
            cost_prev: [0.0; NP],
            used_mark: [0; NP],
            prev_price: [None; NP],
            pot_names: Default::default(),
            session_no: 0,
        };
        t.new_session();
        t
    }

    pub fn now(&self) -> f64 {
        self.fake_now.unwrap_or_else(sys_now)
    }

    fn log(&mut self, s: String) {
        self.logs.push(s);
    }

    pub fn say(&mut self, text: impl Into<String>, sec: f64) {
        self.alert = Some((text.into(), self.now() + sec));
    }

    pub fn cfg_f(&self, key: &str) -> f64 {
        let v = self.cfg.get(key);
        if !truthy(v) {
            return 0.0;
        }
        v.and_then(|v| v.as_f64()).unwrap_or(0.0)
    }
    fn cfg_or(&self, key: &str, dflt: f64) -> f64 {
        self.cfg.get(key).and_then(|v| v.as_f64()).unwrap_or(dflt)
    }
    pub fn has(&self, key: &str) -> bool {
        truthy(self.cfg.get(key))
    }
    pub fn name(&self, k: usize) -> String {
        self.cfg.get(&format!("{}_name", KS[k])).and_then(|v| v.as_str()).unwrap_or("").to_string()
    }
    pub fn cost_of(&self, k: usize) -> f64 {
        self.cost_prev[k] + (self.used(k) - self.used_mark[k]) as f64 * self.price(k)
    }
    pub fn span_cost(&self, k: usize, from: i64, to: i64) -> f64 {
        let m = self.used_mark[k];
        let after = (to - from.max(m)).max(0) as f64;
        let before = (to.min(m) - from).max(0) as f64;
        after * self.price(k) + before * self.prev_price[k].unwrap_or(self.price(k))
    }
    pub fn pot_label(&self, k: usize) -> String {
        let mut v = self.pot_names[k].clone();
        let cur = self.name(k);
        if !cur.is_empty() && v.last() != Some(&cur) {
            v.push(cur);
        }
        v.join("→")
    }
    pub fn ks(&self) -> Vec<usize> {
        (0..NP).filter(|&k| k < 2 || !self.name(k).is_empty()).collect()
    }
    pub fn extra_label(&self) -> String {
        (2..NP).filter(|&k| !self.name(k).is_empty() || self.used(k) > 0).map(|k| format!("{} {}", self.pot_label(k), self.used(k))).collect::<Vec<_>>().join("、")
    }
    pub fn cost_all(&self) -> f64 {
        (0..NP).map(|k| self.cost_of(k)).sum()
    }
    pub fn price(&self, k: usize) -> f64 {
        self.cfg.get(&format!("{}_price", KS[k])).and_then(|v| v.as_f64()).unwrap_or(0.0)
    }
    fn region(&self, k: usize) -> bool {
        self.has(&format!("{}_region", KS[k]))
    }

    pub fn new_session(&mut self) {
        self.acc = 0.0;
        self.run_start = 0.0;
        self.started_at = None;
        self.gain = 0;
        self.hist = vec![];
        self.levelups = 0;
        self.deaths = 0;
        self.qs_used = [0; NP];
        self.bag_used = [0; NP];
        self.meso_anchor = None;
        self.inc_done = None;
        self.seg_pending = false;
        self.sold = 0;
        self.pause_up = None;
        self.sale_ask = None;
        self.meso_drop = None;
        self.pend_dec = None;
        self.reject_n = 0;
        self.pre_inc = None;
        self.rej_run = None;
        self.cap_since = [None; NP];
        self.series = vec![];
        self.curve = vec![];
        self.qs_sus_cnt = [0; NP];
        self.last_drop = Default::default();
        self.recon_acc = [0; NP];
        self.recon_add = [0; NP];
        self.anchor_used = [0; NP];
        self.seg_base = [None; NP];
        self.seg_buy = [0; NP];
        self.seg_t = self.now();
        self.cost_prev = [0.0; NP];
        self.used_mark = [0; NP];
        self.prev_price = [None; NP];
        self.pot_names = Default::default();
        self.session_no += 1;
    }

    pub fn snapshot(&self, a: f64) -> Snap {
        (a, self.gain, self.used(0), self.used(1), self.income(), std::array::from_fn(|k| self.used(k)))
    }

    pub fn curve_pt(&self, a: f64) -> CurvePt {
        (a, self.gain, self.used(0), self.used(1), self.income(), self.cost_all(), self.sold)
    }

    pub fn curve_now(&self) -> Vec<CurvePt> {
        let a = self.active();
        let mut v = self.curve.clone();
        if v.last().map_or(true, |l| a - l.0 >= 1.0) {
            v.push(self.curve_pt(a));
        }
        v
    }

    pub fn recent(&self, a: f64, span: f64) -> (Snap, Option<Snap>, f64) {
        let cur = self.snapshot(a);
        if self.series.is_empty() {
            return (cur, None, 0.0);
        }
        let mut base = self.series[0];
        for snap in &self.series {
            if snap.0 > a - span {
                break;
            }
            base = *snap;
        }
        (cur, Some(base), a - base.0)
    }

    pub fn active(&self) -> f64 {
        self.acc + if self.counting() { self.now() - self.run_start } else { 0.0 }
    }
    pub fn counting(&self) -> bool {
        self.running && !self.paused
    }
    pub fn used(&self, k: usize) -> i64 {
        self.qs_used[k] + self.bag_used[k]
    }
    pub fn stuck(&self, k: usize) -> bool {
        self.cap_since[k].map_or(false, |t| self.now() - t > 300.0)
    }
    pub fn is_qs_mode(&self, k: usize) -> bool {
        self.region(k) && !self.stuck(k)
    }
    pub fn est_total(&self, k: usize) -> Option<i64> {
        let (t, q, a, _) = self.total[k]?;
        Some((t - (self.qs_all[k] - q) + (self.qs_add[k] - a)).max(0))
    }
    pub fn inv_ready(&self, k: Option<usize>) -> bool {
        let ks = match k {
            Some(k) => vec![k],
            None => self.ks(),
        };
        self.has("inv_region") && ks.iter().any(|&x| self.tpl_exists[x])
    }

    pub fn start_meso(&mut self) {
        if let Some(p) = &self.meso_pend {
            if Some(p.0) != self.meso {
                return;
            }
        }
        if self.meso_anchor.is_none() {
            if let Some(m) = self.meso {
                if self.now() - self.meso_t < 5.0 {
                    self.meso_anchor = Some((m, self.active(), Some(self.meso_t)));
                    self.ask_sale();
                }
            }
        }
    }

    fn ask_sale(&mut self) {
        if let Some((_, up)) = self.pause_up.take() {
            if up > 0 {
                *self.sale_ask.get_or_insert(0) += up;
                self.log(format!("暫停期間楓幣多了 {}，問是不是賣掉落物", comma(up)));
            }
        }
    }

    pub fn answer_sale(&mut self, yes: bool) {
        let Some(v) = self.sale_ask.take() else { return };
        if yes {
            self.sold += v;
            self.log(format!("賣掉落物 {}，算進收益", comma(v)));
        } else {
            self.log(format!("暫停期間多的 {} 不是賣掉落物，不算", comma(v)));
        }
    }

    pub fn close_seg(&mut self, act_end: f64) {
        let (Some(ma), Some(m)) = (self.meso_anchor, self.meso) else { return };
        let d = self.inc_done.unwrap_or((0, 0.0));
        self.inc_done = Some((d.0 + m - ma.0, d.1 + (act_end - ma.1).max(0.0)));
        self.meso_anchor = None;
        self.pause_up = Some((m, 0));
    }

    pub fn feed_meso(&mut self, v: i64, bag_id: i64) -> &'static str {
        let now = self.now();
        match &mut self.meso_pend {
            Some(p) if p.0 == v => {
                p.1 += 1;
                p.2.insert(bag_id);
            }
            p => {
                let same = matches!(p, Some(q) if (v - q.0).abs() as f64 <= (q.0.abs() as f64) * 0.05
                    && v.to_string().len() == q.0.to_string().len());
                let mut ids = if same { p.as_ref().unwrap().2.clone() } else { BTreeSet::new() };
                ids.insert(bag_id);
                *p = Some((v, 1, ids));
            }
        }
        let (pc, pids) = {
            let p = self.meso_pend.as_ref().unwrap();
            (p.1, p.2.len())
        };
        let last = self.meso;
        let big = match last {
            Some(l) if l != 0 => {
                (v - l).abs() as f64 > l as f64 * 0.5 || (v.to_string().len() as i64 - l.to_string().len() as i64).abs() >= 2
            }
            _ => false,
        };
        let need = if self.counting() && self.meso_anchor.is_none() { 3 } else { 2 };
        if pc < need || (big && pids < 2) {
            return "wait";
        }
        if Some(v) != last {
            let tail = match last {
                Some(l) => format!("（上次 {}，{}）", comma(l), comma_signed(v - l)),
                None => "（第一次讀到）".into(),
            };
            self.log(format!("楓幣 {}{tail}", comma(v)));
        }
        let spent = matches!(last, Some(l) if v < l)
            && ((self.paused && self.seg_pending) || (self.counting() && self.meso_anchor.is_some()));
        if spent {
            self.seg_pending = false;
            let end = if self.paused { self.acc } else { self.active() };
            self.close_seg(end);
            self.meso_drop = Some((last.unwrap(), v, now));
        } else if let Some((old, low, t)) = self.meso_drop {
            if v >= old && now - t < 30.0 && self.inc_done.is_some() && self.meso_anchor.map_or(false, |a| a.0 == low) {
                self.inc_done.as_mut().unwrap().0 -= old - low;
                self.log(format!("楓幣 {}：剛才的 {} 是讀錯，不算花掉", comma(v), comma(low)));
            }
            if v != low {
                self.meso_drop = None;
            }
        }
        self.meso = Some(v);
        self.meso_t = now;
        if spent && self.paused {
            let l = last.unwrap();
            self.log(format!("楓幣 {} 比暫停前的 {} 少：是暫停期間花掉的，不算收益，用 {} 當暫停前的結尾", comma(v), comma(l), comma(l)));
        } else if spent {
            let l = last.unwrap();
            self.log(format!("楓幣 {} 比上次的 {} 少：是花掉的，不算收益，從 {} 重新起算", comma(v), comma(l), comma(v)));
        } else if self.paused && self.seg_pending {
            self.seg_pending = false;
            let a = self.acc;
            self.close_seg(a);
            self.log(format!("楓幣 {} 當作暫停前的結尾，暫停期間的楓幣增減不算", comma(v)));
        }
        if self.meso_anchor.is_none() {
            if let Some((l, up)) = self.pause_up.as_mut() {
                *up += (v - *l).max(0);
                *l = v;
            }
        }
        if self.counting() && self.meso_anchor.is_none() {
            self.meso_anchor = Some((v, self.active(), Some(self.meso_t)));
            self.log(format!("楓幣 {} 當作這次的起點", comma(v)));
            self.ask_sale();
        }
        "ok"
    }

    fn meso_settled(&mut self, v: i64) -> bool {
        let now = self.now();
        let Some(l) = self.meso else { return false };
        if digits_differ(l, v) < 2 {
            self.meso_cand = None;
            return false;
        }
        match &mut self.meso_cand {
            Some(c) if (v - c.0).abs() as f64 <= (c.0 as f64 * 0.1).max(2_000_000.0) => {
                c.0 = v;
                c.1 += 1;
            }
            c => *c = Some((v, 1, now)),
        }
        let c = self.meso_cand.unwrap();
        if c.1 >= 5 && now - c.2 >= 10.0 {
            self.meso_cand = None;
            return true;
        }
        false
    }

    pub fn meso_near(&self, v: i64) -> bool {
        self.meso.map_or(false, |l| (v - l).abs() as f64 <= (l as f64 * 0.1).max(2_000_000.0))
    }

    pub fn income(&self) -> Option<i64> {
        let d = self.inc_done;
        match (self.meso_anchor, self.meso) {
            (Some(ma), Some(m)) => {
                let dm = m - ma.0;
                Some(d.map_or(dm, |d| d.0 + dm))
            }
            _ => d.map(|d| d.0),
        }
    }

    pub fn income_secs(&self, a: f64) -> f64 {
        self.inc_done.map_or(0.0, |d| d.1) + self.meso_anchor.map_or(0.0, |ma| a - ma.1)
    }

    pub fn level(&self) -> Option<u32> {
        self.need.and_then(exp_table::level_of)
    }

    fn snap_need(&mut self) {
        if let Some(l) = self.level() {
            self.need = exp_table::need(l).map(|n| n as f64);
        }
    }

    pub fn feed_exp(&mut self, exp: i64, pct: Option<f64>, trusted: bool) -> bool {
        let counting = self.counting();
        let now = self.active();
        let need_nz = self.need.filter(|&n| n != 0.0);
        let mut need_est = pct.filter(|&p| p >= 1.0).map(|p| exp as f64 * 100.0 / p);
        let mut pct = pct;

        if self.exp_last.is_none() {
            if !trusted && self.pend_first != Some((exp, pct)) {
                self.pend_first = Some((exp, pct));
                return false;
            }
            self.pend_first = None;
            return self.accept(exp, pct, need_est, 0, false, false, counting, now);
        }
        let (lexp, lpct) = self.exp_last.unwrap();
        if exp == lexp {
            if let (false, Some(need), Some(p)) = (trusted, need_nz, pct) {
                if (p - exp as f64 * 100.0 / need).abs() > 1.0 {
                    pct = lpct;
                    need_est = None;
                }
            }
            return self.accept(exp, pct, need_est, 0, false, false, counting, now);
        }
        if exp > lexp {
            let d = exp - lexp;
            if let (false, Some(need)) = (trusted, need_nz) {
                let p = pct.unwrap_or(0.0);
                let ne_nz = need_est.filter(|&n| n != 0.0);
                let ok_need = ne_nz.map_or(false, |ne| p >= 3.0 && (ne - need).abs() / need <= 0.03);
                if ne_nz.is_some() && p >= 3.0 && !ok_need {
                    return self.reject(exp, pct, need_est, false, true);
                }
                if d as f64 > 0.2 * need && !(ok_need && lpct.map_or(false, |lp| ((p - lp) - d as f64 * 100.0 / need).abs() < 1.0)) {
                    return self.reject(exp, pct, need_est, false, true);
                }
            }
            return self.accept(exp, pct, need_est, d, false, true, counting, now);
        }
        let levelup = if let (Some(p), Some(lp0)) = (pct, lpct) {
            let lp = need_nz.map_or(lp0, |n| lexp as f64 * 100.0 / n);
            lp - p > 15.0
        } else {
            (exp as f64) < lexp as f64 * 0.5
        };
        if !trusted {
            let ok = matches!(self.pend_dec, Some(pd) if pct.is_none() || pd.1.is_none() || (pd.1.unwrap() - pct.unwrap()).abs() < 2.0);
            if !ok {
                self.pend_dec = Some((exp, pct));
                return self.reject(exp, pct, need_est, true, false);
            }
            if let (Some(need), Some(ne), Some(p)) = (need_nz, need_est.filter(|&n| n != 0.0), pct) {
                if p >= 3.0 {
                    if levelup && ne < need * 0.99 {
                        return self.reject(exp, pct, need_est, false, false);
                    }
                    if !levelup && (ne - need).abs() / need > 0.03 {
                        return self.reject(exp, pct, need_est, false, false);
                    }
                }
            }
        }
        let pre = self.pre_inc.take();
        if let (false, Some(pre)) = (levelup, pre) {
            if exp >= pre.0 && self.now() - pre.2 < 60.0 {
                if counting {
                    self.hist.truncate(pre.1);
                }
                self.log(format!("EXP {} → {}：上一筆是讀錯，已修正", comma(lexp), comma(exp)));
                return self.accept(exp, pct, need_est, exp - lexp, false, false, counting, now);
            }
        }
        if levelup {
            if counting {
                self.levelups += 1;
            }
            self.log(format!("EXP {}[{}%] → {}[{}%]：升級", comma(lexp), py_opt(lpct), comma(exp), py_opt(pct)));
            let add = need_nz.map_or(0, |n| pround(n - lexp as f64)) + exp;
            return self.accept(exp, pct, need_est, add, true, false, counting, now);
        }
        if counting {
            self.deaths += 1;
        }
        self.log(format!("EXP {}[{}%] → {}[{}%]：死亡，掉了 {}", comma(lexp), py_opt(lpct), comma(exp), py_opt(pct), comma(lexp - exp)));
        self.accept(exp, pct, need_est, exp - lexp, false, false, counting, now)
    }

    #[allow(clippy::too_many_arguments)]
    fn accept(&mut self, exp: i64, pct: Option<f64>, need_est: Option<f64>, add: i64, levelup: bool, inc: bool, counting: bool, now: f64) -> bool {
        if inc {
            self.pre_inc = Some((self.exp_last.unwrap().0, self.hist.len(), self.now()));
        }
        self.exp_last = Some((exp, pct));
        self.reject_n = 0;
        self.rej_run = None;
        self.pend_dec = None;
        if levelup {
            let next = self.level().and_then(|l| exp_table::need(l + 1)).map(|n| n as f64);
            self.need = next.or(need_est);
        } else if let Some(ne) = need_est.filter(|&n| n != 0.0) {
            if pct.unwrap_or(0.0) >= 3.0 || self.need.map_or(true, |n| n == 0.0) {
                self.need = Some(ne);
            }
        }
        self.snap_need();
        if counting {
            self.gain += add;
            self.hist.push((now, self.gain));
        }
        true
    }

    fn reject(&mut self, exp: i64, pct: Option<f64>, need_est: Option<f64>, keep_pend: bool, up: bool) -> bool {
        if !keep_pend {
            self.pend_dec = None;
        }
        let scale = self.need.filter(|&n| n != 0.0).unwrap_or(exp as f64);
        self.rej_run = match self.rej_run {
            Some(r) if up && exp >= r.1 && (exp - r.1) as f64 <= 0.05 * scale => Some((r.0, exp, r.2 + 1)),
            _ => {
                if up {
                    Some((exp, exp, 1))
                } else {
                    None
                }
            }
        };
        self.reject_n += 1;
        if self.reject_n >= 5 && self.off_table(exp, pct, need_est) {
            self.reject_n = 0;
            self.rej_run = None;
            self.pend_dec = None;
            return false;
        }
        if self.reject_n >= 5 {
            let last = self.exp_last.map_or("None".to_string(), |(e, p)| format!("({e}, {})", py_opt(p)));
            self.log(format!("EXP 連續 5 次對不上（{last} → {}[{}%]），重新當基準", comma(exp), py_opt(pct)));
            if up && self.rej_run.map_or(false, |r| r.2 >= 5) && self.counting() {
                if let Some((lexp, lpct)) = self.exp_last {
                    let d = exp - lexp;
                    let n = need_est.filter(|&n| n != 0.0).or(self.need).filter(|&n| n != 0.0);
                    if let (Some(p), Some(lp), Some(n)) = (pct, lpct, n) {
                        if ((p - lp) - d as f64 * 100.0 / n).abs() < 1.0 {
                            self.gain += d;
                            let a = self.active();
                            self.hist.push((a, self.gain));
                        }
                    }
                }
            }
            self.exp_last = Some((exp, pct));
            if need_est.map_or(false, |n| n != 0.0) {
                self.need = need_est;
                self.snap_need();
            }
            self.reject_n = 0;
            self.pend_dec = None;
            self.rej_run = None;
            self.pre_inc = None;
        }
        false
    }

    fn off_table(&self, exp: i64, pct: Option<f64>, need_est: Option<f64>) -> bool {
        let Some(l) = self.level() else { return false };
        if exp_table::need(l + 1).map_or(false, |n| exp > n) {
            return true;
        }
        match (need_est.filter(|&n| n != 0.0), pct) {
            (Some(ne), Some(p)) if p >= 3.0 => exp_table::level_of(ne).map_or(true, |nl| nl < l),
            _ => false,
        }
    }

    fn qs_use(&mut self, k: usize, n: i64, sus: bool) {
        let c = n.min(self.qs_credit[k]);
        self.qs_credit[k] = 0;
        let n = n - c;
        if n <= 0 {
            return;
        }
        self.qs_all[k] += n;
        let sus = sus || n >= 20;
        if sus {
            self.qs_sus[k] += n;
        }
        if self.counting() {
            self.qs_used[k] += n;
            if sus {
                self.qs_sus_cnt[k] += n;
            }
        }
    }

    fn qs_drop(&mut self, k: usize, last: i64, val: i64, now: f64) -> i64 {
        match self.qs_rise[k] {
            Some((before, t)) if now - t <= 60.0 => {
                if val <= before {
                    self.qs_rise[k] = None;
                }
                (before - val).max(0)
            }
            _ => {
                self.qs_rise[k] = None;
                last - val
            }
        }
    }

    pub fn feed_qs(&mut self, k: usize, val: i64) {
        let (last, now) = (self.qs[k], self.now());
        if self.qs_wrong[k].map_or(false, |(w, t)| w == val && now - t < 600.0) {
            return;
        }
        let gap = now - self.qs_seen[k];
        self.qs_seen[k] = now;
        if gap > self.blind_sec() {
            self.qs_blind_t[k] = now;
        }
        let step = (15f64).max((now - self.qs_t[k]) / 60.0 * 90.0);
        let mut val = val;
        let sure = self.qs_fell[k] || self.qs_ok[k];
        let in_bag = |x: i64| self.inv_cells[k].iter().any(|&c| (c - x).abs() <= 2);
        let fix = last.filter(|&l| val > l).and_then(|l| drop_one_digit(val, l, step)).filter(|&c| {
            let l = last.unwrap();
            (sure && l as f64 > step) || (self.qs_fell[k] && !in_bag(val) && in_bag(c))
        });
        match fix {
            Some(c) => {
                if !self.qs_fixing[k] {
                    self.qs_fixing[k] = true;
                    let name = self.name(k);
                    self.log(format!("{name} 快捷欄讀到 {val}，像是 {} 多讀了一位數，當成 {c}", last.unwrap()));
                }
                val = c;
            }
            None => self.qs_fixing[k] = false,
        }
        if last.is_none() || last == Some(val) {
            if last.is_none() {
                self.qs_fell[k] = false;
                self.qs_ok[k] = false;
            }
            self.qs[k] = Some(val);
            self.qs_pend[k] = None;
            self.qs_t[k] = now;
            self.qs_credit[k] = 0;
            return;
        }
        let last = last.unwrap();
        let mut p = self.qs_pend[k];
        if p.is_some() && gap > self.blind_sec() {
            p = None;
            self.qs_pend[k] = None;
        }
        if let Some(pp) = p {
            if val < pp.0 && pp.0 < last && (last - val) as f64 <= step {
                let n = self.qs_drop(k, last, pp.0, now);
                self.qs_use(k, n, false);
                self.qs_fell[k] = true;
                self.qs[k] = Some(pp.0);
                self.qs_pend[k] = Some((val, 1));
                self.qs_t[k] = now;
                return;
            }
        }
        let cnt = match p {
            Some(pp) if pp.0 == val => {
                self.qs_pend[k] = Some((val, pp.1 + 1));
                pp.1 + 1
            }
            _ => {
                self.qs_pend[k] = Some((val, 1));
                return;
            }
        };
        let cap = self.cfg_f("stack_max") as i64;
        let name = self.name(k);
        if val > last {
            if !self.qs_fell[k] && cnt < 8 {
                return;
            }
            if last as f64 <= step && self.qs_fell[k] {
                let extra = if cap != 0 && (cap as f64 - step) <= val as f64 && val <= cap { cap - val } else { 0 };
                let n = (last + extra).max(0);
                self.qs_use(k, n, true);
                self.qs_rise[k] = None;
                self.qs_fell[k] = true;
                self.qs_ok[k] = false;
                self.qs_switched[k] = true;
                self.log(format!("{name} 快捷欄 {last} → {val}（這格用完換下一格，算用掉 {n}）"));
            } else {
                self.qs_fell[k] = false;
                self.qs_ok[k] = false;
                self.qs_rise[k] = if val - last < 50 {
                    Some((self.qs_rise[k].filter(|r| now - r.1 <= 60.0).map_or(last, |r| r.0.min(last)), now))
                } else {
                    None
                };
                self.log(format!("{name} 快捷欄 {last} → {val}（變多，不算用量）"));
            }
        } else if (last - val) as f64 <= step {
            let n = self.qs_drop(k, last, val, now);
            self.qs_use(k, n, false);
            self.qs_fell[k] = true;
        } else if cnt < 8 {
            return;
        } else {
            self.qs_rise[k] = None;
            self.qs_fell[k] = false;
            self.qs_ok[k] = false;
            self.qs_switched[k] = true;
            self.log(format!("{name} 快捷欄 {last} → {val}（一下子少太多，當成換格顯示，不算用量）"));
        }
        self.qs[k] = Some(val);
        self.qs_pend[k] = None;
        self.qs_t[k] = now;
        self.qs_credit[k] = 0;
    }

    fn check_qs_bag(&mut self, k: usize, stacks: &[Option<i64>], raw: Option<i64>, bag_id: i64) {
        if !self.region(k) || self.now() - self.qs_seen[k] > self.blind_sec() {
            return;
        }
        let wrong = raw.filter(|&r| self.qs_wrong[k].map_or(false, |w| w.0 == r));
        let Some(v) = wrong.or(self.qs[k]) else { return };
        let cells: Vec<i64> = stacks.iter().flatten().copied().collect();
        if cells.is_empty() || cells.len() != stacks.len() {
            return;
        }
        let pend = self.qs_pend[k].map(|p| p.0);
        let near = |x: i64| cells.iter().any(|&c| (c - x).abs() <= 2);
        if near(v) || raw.map_or(false, near) || pend.map_or(false, near) {
            self.qs_ok[k] = true;
            self.qs_bad[k].0 = 0;
            return;
        }
        if self.qs_bad[k].1 == bag_id || self.inv_slots[k] != Some(cells.len() as i64) {
            return;
        }
        self.qs_ok[k] = false;
        self.qs_bad[k].0 += 1;
        if self.qs_bad[k].0 < 2 {
            return;
        }
        self.qs_bad[k] = (0, bag_id);
        if let Some(t) = self.total[k] {
            self.qs_sus[k] = self.qs_sus[k].max(self.qs_all[k] - t.1);
            self.qs_sus_cnt[k] = self.qs_sus_cnt[k].max(self.qs_used[k] - self.anchor_used[k]);
        }
        let name = self.name(k);
        let list = cells.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("、");
        let cap = self.cfg_f("stack_max") as i64;
        let part: Vec<i64> = cells.iter().copied().filter(|&c| c != cap).collect();
        let pool = if part.is_empty() { &cells } else { &part };
        let vs = v.to_string();
        let sub: Vec<i64> = pool
            .iter()
            .copied()
            .filter(|c| {
                let cs = c.to_string();
                cs.len().min(vs.len()) >= 2 && (cs.contains(&vs) || vs.contains(&cs))
            })
            .collect();
        let one_off = |c: i64| {
            let cs = c.to_string();
            cs.len() == vs.len() && cs.bytes().zip(vs.bytes()).filter(|(a, b)| a != b).count() <= 1
        };
        let far = sub.is_empty() && !cells.iter().any(|&c| one_off(c));
        let pool = if sub.is_empty() { pool } else { &sub };
        let c = *pool.iter().min_by_key(|&&c| (c - v).abs()).unwrap();
        self.log(format!(
            "{name} 快捷欄讀到 {}，背包裡的格子是 {list}，對不上 → 快捷欄讀錯了，改用背包裡的 {}（之前多算的用量會在背包總數確認後退回）",
            v,
            c
        ));
        let again = far && self.now() - self.qs_far_last[k] < 300.0;
        if far {
            self.qs_far_last[k] = self.now();
        }
        if again && self.now() - self.qs_far_t[k] > 600.0 {
            self.qs_far_t[k] = self.now();
            let msg = format!("{name} 快捷欄的數字跟背包對不上，可能框錯位置了，請打開背包、切到消耗欄，再按設定裡的「自動框選」");
            self.say(msg.clone(), 15.0);
            self.log(msg);
        }
        self.qs_wrong[k] = Some((v, self.now()));
        self.qs[k] = Some(c);
        self.qs_pend[k] = None;
        self.qs_rise[k] = None;
        self.qs_t[k] = self.now();
        self.qs_credit[k] = 0;
        self.qs_fell[k] = false;
        self.qs_ok[k] = true;
    }

    pub fn blind_sec(&self) -> f64 {
        let o = self.cfg_f("ocr_sec");
        (8f64).max(2.0 * if o == 0.0 { 3.0 } else { o } + 2.0)
    }

    pub fn rate_cap(&self, k: usize) -> f64 {
        let s = &self.series;
        if s.len() < 2 || s.last().unwrap().0 - s[0].0 < 600.0 {
            return 90.0;
        }
        let used = |x: &Snap| match k {
            0 => x.2,
            1 => x.3,
            _ => x.5[k],
        };
        let (mut peak, mut j) = (0i64, 0usize);
        for i in 0..s.len() {
            while s[i].0 - s[j].0 > 600.0 {
                j += 1;
            }
            peak = peak.max(used(&s[i]) - used(&s[j]));
        }
        (peak as f64 / 10.0 * 2.0).clamp(20.0, 90.0)
    }

    fn plausible(&self, k: usize, secs: f64) -> f64 {
        (15f64).max(secs.max(0.0) / 60.0 * self.rate_cap(k))
    }

    fn qs_pending(&self, k: usize) -> i64 {
        match (self.qs[k], self.qs_pend[k]) {
            (Some(q), Some(p)) => {
                let d = (q - p.0).max(0);
                if d as f64 <= self.plausible(k, self.now() - self.qs_t[k]) { d } else { 0 }
            }
            _ => 0,
        }
    }

    fn qs_live(&self, k: usize) -> bool {
        self.is_qs_mode(k) && self.qs[k].is_some() && self.now() - self.qs_t[k] <= self.blind_sec()
    }

    fn drop_back(&self, k: usize, ld: &Drop, amt: i64, now: f64) -> bool {
        let tol = 5.max(ld.miss / 50);
        (amt - ld.miss).abs() <= tol || (ld.n > 0 && amt < ld.miss && (ld.miss - amt) as f64 <= self.plausible(k, now - ld.t))
    }

    pub fn set_total(&mut self, k: usize, tv: i64, slots: Option<Slots>) {
        let now = self.now();
        let name = self.name(k);
        if let Some(mut est) = self.est_total(k) {
            let prev = self.total[k].unwrap();
            let bag = !self.is_qs_mode(k);
            let qs_live = self.qs_live(k);
            let pend = if qs_live { self.qs_pending(k) } else { 0 };
            let plaus = self.plausible(k, now - prev.3);
            let since = self.qs_all[k] - prev.1;
            let amt = tv - (est - pend);
            if amt > 5 {
                let ld = self.last_drop[k].clone().filter(|d| now - d.t < 1800.0 && self.drop_back(k, d, amt, now));
                let (explain, roll) = if bag {
                    (false, 0)
                } else if !self.qs_ok[k] {
                    (since >= amt - 5, self.qs_used[k] - self.anchor_used[k])
                } else {
                    (self.qs_sus[k] >= amt - 5, self.qs_sus_cnt[k])
                };
                let roll = roll.clamp(0, amt);
                if explain {
                    self.qs_used[k] -= roll;
                    let tail = if roll != 0 { format!("，退回用量 {}", comma(roll)) } else { String::new() };
                    self.log(format!("{name} 背包總數 {}，預估 {}（多 {}）→ 快捷欄之前讀錯多扣了{tail}", comma(tv), comma(est), comma(amt)));
                    self.qs_rebase(k);
                } else if let Some(ld) = ld {
                    let back = amt.min(ld.n);
                    self.bag_used[k] -= back;
                    self.qs_credit[k] = (self.qs_credit[k] - amt).max(0);
                    self.last_drop[k] = None;
                    let tail = if back > 0 { format!("→ 退回用量 {}", comma(back)) } else { "→ 不算補貨".into() };
                    self.log(format!("{name} 背包總數 {}，上次少掉的 {} 又出現了 {}（當時被擋住）{tail}", comma(tv), comma(ld.miss), comma(amt)));
                } else {
                    self.last_rise[k] = Some((amt, est, now, self.qs_all[k], self.qs_add[k]));
                    self.seg_buy[k] += amt;
                    self.log(format!("{name} 背包總數 {}，預估 {}（多 {}）→ 不算用量，當作新的總數", comma(tv), comma(est), comma(amt)));
                }
            } else {
                if let Some((amt0, est0, t0, q0, a0)) = self.last_rise[k] {
                    if now - t0 < 1800.0 {
                        let est0_now = est0 - (self.qs_all[k] - q0) + (self.qs_add[k] - a0);
                        let back = tv - est0_now <= 5 && (est0_now - tv) as f64 <= self.plausible(k, now - t0);
                        if back && (qs_live || (est - tv) as f64 > plaus) {
                            self.last_rise[k] = None;
                            self.seg_buy[k] -= amt0;
                            self.log(format!("{name} 背包總數 {}，回到變多之前的數字 → 上次多出來的 {} 是讀錯，不算用量", comma(tv), comma(amt0)));
                            est = est0_now;
                        } else if (tv - est).abs() <= 5 {
                            self.last_rise[k] = None;
                        }
                    }
                }
                let miss = est - pend - tv;
                if miss != 0 {
                    self.log(format!("{name} 背包總數 {}，預估 {}（差 {}）", comma(tv), comma(est - pend), comma_signed(-miss)));
                }
                if miss > 5 || (miss > 0 && (bag && self.running || !bag && !qs_live)) {
                    if prev.3 < self.fill_from || miss as f64 > plaus {
                        self.last_drop[k] = Some(Drop { miss, t: now, n: 0 });
                        self.log(format!("{name} 背包少了 {}，不像是喝掉的，不算用量", comma(miss)));
                    } else {
                        let frac = (self.active() - self.total_act[k]).max(0.0) / (now - prev.3).max(1.0);
                        let n = miss.min(pround(miss as f64 * frac)).max(0);
                        self.bag_used[k] += n;
                        self.last_drop[k] = Some(Drop { miss, t: now, n });
                        if bag {
                        } else if !qs_live {
                            self.qs_credit[k] += miss;
                            self.qs_pend[k] = None;
                            if n != 0 {
                                self.log(format!("{name} 快捷欄讀不到的這段時間用掉 {} 瓶（用背包的數字補算）", comma(n)));
                            }
                        } else {
                            self.log(format!("{name} 背包少了 {}，快捷欄只算到 {} → 快捷欄漏算，補算 {}", comma(miss), comma(since), comma(n)));
                            let whole = slots.map_or(true, |s| s.0.map_or(true, |a| s.1 >= a));
                            if since == 0 && whole && self.qs_blind_t[k] < prev.3 && now - prev.3 >= 20.0 {
                                self.qs_flat_strike(k);
                            }
                        }
                    }
                } else if !bag && self.counting() {
                    if self.recon_acc[k] == 0 {
                        self.drift_from[k] = (prev.3, prev.1);
                    }
                    let acc = (self.recon_acc[k] + miss).max(-self.recon_add[k] - 5);
                    self.recon_acc[k] = acc;
                    if acc > 5 {
                        self.bag_used[k] += acc;
                        self.recon_add[k] += acc;
                        self.recon_acc[k] = 0;
                        self.log(format!("{name} 對帳：背包實際少掉的比算到的多 {} 瓶（快捷欄漏算），補算", comma(acc)));
                        let (t0, q0) = self.drift_from[k];
                        let whole = slots.map_or(true, |s| s.0.map_or(true, |a| s.1 >= a));
                        if self.qs_all[k] == q0 && whole && self.qs_blind_t[k] < t0 && now - t0 >= 20.0 {
                            self.qs_flat_strike(k);
                        }
                    } else if acc < -5 && self.recon_add[k] > 0 {
                        let back = self.recon_add[k].min(-acc);
                        self.bag_used[k] -= back;
                        self.recon_add[k] -= back;
                        self.recon_acc[k] += back;
                        self.log(format!("{name} 對帳：算到的用量比背包少掉的多 {} 瓶，退回之前補算的 {}", comma(-acc), comma(back)));
                    }
                }
            }
        } else {
            self.log(format!("{name} 背包總數 {}（第一次讀到，當作起點）", comma(tv)));
        }
        self.total[k] = Some((tv, self.qs_all[k], self.qs_add[k], now));
        self.total_act[k] = self.active();
        self.anchor_used[k] = self.qs_used[k];
        self.anchor_cells[k] = self.inv_cells[k].clone();
        self.qs_sus[k] = 0;
        self.qs_sus_cnt[k] = 0;
        self.qs_switched[k] = false;
        self.seg_check(k, tv);
    }

    fn qs_rebase(&mut self, k: usize) {
        let Some(v) = self.qs[k] else { return };
        let cells = &self.inv_cells[k];
        if cells.is_empty() || cells.iter().any(|&c| (c - v).abs() <= 2) {
            return;
        }
        let cap = self.cfg_f("stack_max") as i64;
        let part: Vec<i64> = cells.iter().copied().filter(|&c| c != cap).collect();
        let pool = if part.is_empty() { cells } else { &part };
        let c = *pool.iter().min_by_key(|&&c| (c - v).abs()).unwrap();
        let name = self.name(k);
        self.log(format!("{name} 快捷欄的 {v} 背包裡沒有，改用背包裡的 {c}"));
        self.qs_wrong[k] = Some((v, self.now()));
        self.qs[k] = Some(c);
        self.qs_pend[k] = None;
        self.qs_rise[k] = None;
        self.qs_t[k] = self.now();
        self.qs_fell[k] = false;
        self.qs_ok[k] = true;
    }

    fn flush_drift(&mut self) {
        for k in self.ks() {
            let acc = self.recon_acc[k];
            if acc > 0 {
                self.bag_used[k] += acc;
                self.recon_add[k] += acc;
                self.recon_acc[k] = 0;
                let name = self.name(k);
                self.log(format!("{name} 對帳：背包比算到的多少了 {} 瓶，補進用量", comma(acc)));
            }
        }
    }

    fn seg_check(&mut self, k: usize, tv: i64) {
        if !self.counting() {
            return;
        }
        let used = self.used(k);
        let Some((t0, u0)) = self.seg_base[k] else {
            self.seg_base[k] = Some((tv, used));
            self.seg_buy[k] = 0;
            return;
        };
        let expect = t0 + self.seg_buy[k] - tv;
        let got = used - u0;
        let now = self.now();
        if (expect - got).abs() > 5 && now - self.seg_check_t[k] > 600.0 {
            self.seg_check_t[k] = now;
            let name = self.name(k);
            self.log(format!(
                "{name} 整段核對：背包從 {} 到 {}（補貨 {}）應該用掉 {}，算到 {}（差 {}）",
                comma(t0),
                comma(tv),
                comma(self.seg_buy[k]),
                comma(expect),
                comma(got),
                comma_signed(got - expect)
            ));
        }
    }

    fn qs_flat_strike(&mut self, k: usize) {
        let now = self.now();
        let (n, last) = self.qs_flat[k];
        let n = if now - last < 1800.0 { n + 1 } else { 1 };
        self.qs_flat[k] = (n, now);
        if n >= 2 && now - self.qs_flat_said[k] > 600.0 {
            self.qs_flat_said[k] = now;
            let name = self.name(k);
            let msg = format!("快捷欄的{name}一段時間未減少，可能用錯藥水，或檢查設定重新框選");
            self.say(msg.clone(), 15.0);
            self.log(msg);
        }
    }

    pub fn feed_inv(&mut self, k: usize, stacks: &[Option<i64>]) -> &'static str {
        let cap = self.cfg_f("stack_max") as i64;
        if stacks.iter().any(|v| v.map_or(true, |v| cap != 0 && v > cap)) {
            return "bad";
        }
        let cells: Vec<i64> = stacks.iter().map(|v| v.unwrap()).collect();
        let tv: i64 = cells.iter().sum();
        let n = stacks.len() as i64;
        self.inv_cells[k] = cells.clone();
        let cnt = match &mut self.inv_pend[k] {
            Some(p) if p.0 == tv && p.1 == n => {
                p.2 += 1;
                p.2
            }
            p => {
                *p = Some((tv, n, 1));
                1
            }
        };
        let pend = if self.qs_live(k) { self.qs_pending(k) } else { 0 };
        let plaus = self.total[k].map_or(15.0, |t| self.plausible(k, self.now() - t.3));
        let need = match self.est_total(k) {
            None => 3,
            Some(e) if tv > e - pend + 5 => 4,
            Some(e) if self.qs_live(k) && self.inv_slots[k].map_or(true, |s| n >= s) && (e - pend - tv) as f64 > plaus => 4,
            _ => 2,
        };
        if cnt < need {
            return "wait";
        }
        match self.hidden_cells(k, tv, n, &cells) {
            Hide::Hold => {}
            Hide::Fill(total, hidden) => {
                let slots = self.inv_slots[k];
                self.set_total(k, total, Some((slots, slots.unwrap_or(n))));
                self.anchor_cells[k].extend(hidden);
            }
            Hide::No => {
                let slots = (self.inv_slots[k], n);
                self.inv_slots[k] = Some(n);
                self.set_total(k, tv, Some(slots));
            }
        }
        "ok"
    }

    fn hidden_cells(&mut self, k: usize, tv: i64, n: i64, cells: &[i64]) -> Hide {
        let now = self.now();
        let (Some(e), Some(slots)) = (self.est_total(k), self.inv_slots[k]) else {
            self.inv_hide[k] = None;
            return Hide::No;
        };
        let prev_t = self.total[k].map_or(now, |t| t.3);
        let qs_live = self.qs_live(k);
        let miss = e - if qs_live { self.qs_pending(k) } else { 0 } - tv;
        if !(n < slots && miss > 5) {
            self.inv_hide[k] = None;
            return Hide::No;
        }
        if let Some((_, est0, t0, q0, a0)) = self.last_rise[k] {
            let est0_now = est0 - (self.qs_all[k] - q0) + (self.qs_add[k] - a0);
            if now - t0 < 1800.0 && tv - est0_now <= 5 && (est0_now - tv) as f64 <= self.plausible(k, now - t0) {
                self.inv_hide[k] = None;
                return Hide::No;
            }
        }
        let plaus = self.plausible(k, now - prev_t);
        let qs_v = self.qs[k].filter(|_| qs_live);
        let qs_seen = |v: i64| cells.iter().any(|&c| (c - v).abs() <= 2);
        let in_use_hidden = qs_v.map_or(false, |v| v > 0 && miss + 5 >= v && !qs_seen(v));
        let no_switch = qs_live && self.qs_blind_t[k] < prev_t && !self.qs_switched[k];
        let too_small = self.anchor_cells[k].iter().min().map_or(false, |&m| miss < m - 2);
        let too_fast = miss as f64 > plaus;
        let still = self.inv_hide[k].is_some();
        if !(still || self.both_drop || in_use_hidden || no_switch || too_small || too_fast) {
            self.inv_hide[k] = None;
            return Hide::No;
        }
        let name = self.name(k);
        if let Some(t0) = self.inv_hide[k] {
            if now - t0 >= 600.0 {
                self.inv_hide[k] = None;
                self.log(format!("{name} 背包一直少了 {}（超過 10 分鐘）→ 當作真的少了（賣掉或放倉庫），不算用量", comma(miss)));
                return Hide::No;
            }
        }
        let mut left: Vec<Option<i64>> = self.anchor_cells[k].iter().map(|&a| Some(a)).collect();
        let mut used = 0;
        let mut fit = true;
        for &c in cells {
            let pick = left
                .iter()
                .enumerate()
                .filter_map(|(i, a)| a.filter(|&a| a + 2 >= c).map(|a| (a - c, i)))
                .min();
            let Some((d, i)) = pick else {
                fit = false;
                break;
            };
            left[i] = None;
            used += d.max(0);
        }
        let mut hidden: Vec<i64> = left.into_iter().flatten().collect();
        let cap = self.cfg_f("stack_max") as i64;
        let bag = !self.is_qs_mode(k);
        if fit && (bag || self.qs_switched[k]) {
            if let Some(i) = hidden.iter().enumerate().filter(|(_, a)| **a != cap).min_by_key(|(_, a)| **a).map(|(i, _)| i) {
                let a = hidden[i];
                if !bag || (used + a) as f64 <= plaus {
                    hidden.remove(i);
                    if bag {
                        used += a;
                    }
                }
            }
        }
        if fit {
            if let Some(v) = qs_v.filter(|&v| !qs_seen(v)) {
                match hidden.iter().enumerate().filter(|(_, a)| **a + 2 >= v).min_by_key(|(_, a)| **a - v).map(|(i, _)| i) {
                    Some(i) => hidden[i] = v,
                    None => fit = false,
                }
            }
        }
        if fit && hidden.is_empty() {
            self.inv_hide[k] = None;
            return Hide::No;
        }
        if self.inv_hide[k].is_none() {
            self.inv_hide[k] = Some(now);
            self.log(format!(
                "{name} 背包只看到 {} 格、共 {}，比預估的 {} 少 {} → 可能有格子被擋住（氣場、提示框），先當作還在",
                n,
                comma(tv),
                comma(e),
                comma(miss)
            ));
        }
        if !fit || used as f64 > plaus {
            return Hide::Hold;
        }
        Hide::Fill(tv + hidden.iter().sum::<i64>(), hidden)
    }

    pub fn forget_potion(&mut self, k: usize) {
        self.total[k] = None;
        self.inv_pend[k] = None;
        self.inv_slots[k] = None;
        self.inv_hide[k] = None;
        self.inv_cells[k].clear();
        self.anchor_cells[k].clear();
        self.seg_base[k] = None;
        self.seg_buy[k] = 0;
        self.last_rise[k] = None;
        self.last_drop[k] = None;
        self.recon_acc[k] = 0;
        self.qs_credit[k] = 0;
        self.qs_sus[k] = 0;
        self.qs_bad[k] = (0, -1);
    }

    pub fn forget_qs(&mut self, k: usize) {
        self.qs[k] = None;
        self.qs_pend[k] = None;
        self.qs_rise[k] = None;
        self.cap_since[k] = None;
        self.qs_fell[k] = false;
        self.qs_ok[k] = false;
        self.qs_fixing[k] = false;
        self.qs_switched[k] = false;
        self.qs_blind_t[k] = self.now();
        self.qs_flat[k] = (0, f64::MIN);
        self.qs_wrong[k] = None;
        self.qs_far_last[k] = f64::MIN;
        self.qs_far_t[k] = f64::MIN;
    }

    pub fn switch_potion(&mut self, k: usize, name: &str, price: f64, mark: Option<i64>) {
        let old = self.name(k);
        let mark = mark.unwrap_or_else(|| self.used(k)).min(self.used(k));
        let n = (mark - self.used_mark[k]).max(0);
        self.cost_prev[k] += n as f64 * self.price(k);
        self.prev_price[k] = Some(self.price(k));
        if n > 0 && !old.is_empty() && old != name && self.pot_names[k].last() != Some(&old) {
            self.pot_names[k].push(old.clone());
        }
        self.used_mark[k] = mark;
        let num = |v: f64| -> Value { if v == v.trunc() { json!(v as i64) } else { json!(v) } };
        self.cfg.insert(format!("{}_name", KS[k]), json!(name));
        self.cfg.insert(format!("{}_price", KS[k]), num(price));
        self.forget_potion(k);
        self.forget_qs(k);
        if old.is_empty() {
            self.log(format!("新增藥水：{name}"));
        } else if name.is_empty() {
            self.log(format!("不算{old}了（用掉的 {} 瓶照{old}的單價算）", comma(self.used(k))));
        } else if n > 0 {
            self.log(format!("{old}換成{name}（之前用掉的 {} 瓶照{old}的單價算）", comma(n)));
        } else {
            self.log(format!("{old}換成{name}"));
        }
    }

    pub fn stats(&mut self) -> Stats {
        let a = self.active();
        let h = a / 3600.0;
        let per = |v: f64| if h > 0.005 { Some(v / h) } else { None };
        let mut s = Stats { active: a, rate: per(self.gain as f64), ..Default::default() };
        let span = self.cfg_or("unit", 600.0);
        if a >= 120.0 && self.hist.len() >= 2 {
            let mut base = self.hist[0];
            for &(t, g) in &self.hist {
                if t > a - span {
                    break;
                }
                base = (t, g);
            }
            let (end_t, end_g) = *self.hist.last().unwrap();
            if end_t - base.0 > 30.0 {
                s.rate_recent = Some((end_g - base.1) as f64 / ((end_t - base.0) / 3600.0));
            }
        }
        if self.hist.len() > 2000 {
            let cut = self.hist.iter().position(|&(t, _)| t > a - 3900.0).unwrap_or(0);
            if cut > 1 {
                self.hist.drain(..cut - 1);
                if let Some((v, idx, t0)) = self.pre_inc {
                    self.pre_inc = Some((v, idx.saturating_sub(cut - 1), t0));
                }
            }
        }
        let cost = self.cost_all();
        s.cost = cost;
        s.sold = self.sold;
        s.cost_hr = per(cost);
        s.eff = if cost > 0.0 { Some(self.gain as f64 / cost * 1e4) } else { None };
        s.hp_hr = per(self.used(0) as f64);
        s.mp_hr = per(self.used(1) as f64);
        s.income = self.income();
        s.profit = s.income.map(|i| (i + self.sold) as f64 - cost);
        s.income_hr = s.income.and_then(|i| per(i as f64));
        s.profit_hr = s.profit.and_then(per);
        s
    }

    pub fn tick(&mut self) -> Stats {
        self.check_idle();
        let s = self.stats();
        let a = s.active;
        if self.counting() && self.curve.last().map_or(true, |l| a - l.0 >= 30.0) {
            let pt = self.curve_pt(a);
            self.curve.push(pt);
        }
        if self.counting() && self.series.last().map_or(true, |l| a - l.0 >= 5.0) {
            let snap = self.snapshot(a);
            self.series.push(snap);
            if self.series.len() > 1000 {
                let keep = self.series.iter().position(|x| x.0 > a - 3900.0).unwrap_or(0);
                if keep > 1 {
                    self.series.drain(..keep - 1);
                }
            }
        }
        s
    }

    pub fn toggle_run(&mut self, back: f64) {
        self.auto_paused = false;
        let now = self.now();
        if !self.running {
            self.running = true;
            self.paused = false;
            self.run_start = now;
            self.exp_move_t = now;
            self.started_at = Some(local_iso());
            self.fill_from = self.fill_from.max(now - 600.0);
            self.recon_acc = [0; NP];
            self.seg_base = [None; NP];
            self.seg_t = now;
            let a = self.active();
            for k in self.ks() {
                self.total_act[k] = self.total_act[k].min(a);
            }
            self.start_meso();
            for k in self.ks() {
                if let Some(t) = self.total[k] {
                    if !self.is_qs_mode(k) && now - t.3 > 600.0 {
                        self.total[k] = None;
                    }
                }
            }
            let need_exp = !self.has("exp_region") && self.exp_last.is_none();
            let no_pot = self.ks().into_iter().any(|k| self.est_total(k).is_none() && !self.region(k));
            let need_meso = self.has("meso_region") && self.meso_anchor.is_none();
            self.say("開始計時", 4.0);
            self.log("===== 開始計時 =====".into());
            if (no_pot || need_meso) && self.inv_ready(None) && !need_exp {
                self.say("開始計時，請打開背包，讓浮窗讀藥水總數和楓幣", 8.0);
            } else if need_exp {
                self.say("開始計時。還沒框選 EXP，請按 ☰ 框選畫面下方的 EXP 數字", 8.0);
            } else if no_pot && !self.hint_pot {
                self.hint_pot = true;
                self.say("開始計時。藥水還沒設定，按 ☰ 框選快捷欄或背包就能算藥水用量", 8.0);
            }
        } else if self.paused {
            self.paused = false;
            self.run_start = now;
            self.exp_move_t = now;
            self.cap_since = [None; NP];
            let no_end = self.seg_pending;
            if no_end {
                self.seg_pending = false;
            } else {
                self.start_meso();
            }
            self.log("繼續計時".into());
            self.recon_acc = [0; NP];
            self.seg_base = [None; NP];
            self.seg_t = now;
            if no_end && self.has("meso_region") && self.meso_anchor.is_some() {
                self.say("繼續計時（暫停期間沒記錄到楓幣，這段的楓幣增減會計算）", 8.0);
            } else if self.has("meso_region") && self.meso_anchor.is_none() && self.inc_done.is_some() {
                self.say("繼續計時，請打開背包確認楓幣", 8.0);
            } else {
                self.say("繼續計時", 4.0);
            }
        } else {
            self.flush_drift();
            self.acc += now - self.run_start - back;
            self.paused = true;
            if back != 0.0 {
                let acc = self.acc;
                self.series.retain(|x| x.0 <= acc);
                self.curve.retain(|x| x.0 <= acc);
                if let Some(ma) = self.meso_anchor {
                    if ma.1 > acc {
                        self.meso_anchor = Some((ma.0, acc, ma.2));
                    }
                }
                for k in self.ks() {
                    self.total_act[k] = self.total_act[k].min(acc);
                }
            }
            self.log("暫停".into());
            if self.meso_anchor.is_some() {
                if now - self.meso_t < 5.0 {
                    let a = self.acc;
                    self.close_seg(a);
                } else {
                    self.seg_pending = true;
                }
            }
            if self.seg_pending {
                self.say("已暫停，請打開背包記錄楓幣，暫停期間的楓幣增減不計算", 8.0);
            } else {
                let m = if self.has("meso_region") { "、楓幣" } else { "" };
                self.say(format!("已暫停，暫停期間的經驗、藥水{m}都不計算"), 4.0);
            }
        }
    }

    pub fn check_idle(&mut self) {
        let mins = self.cfg_f("auto_pause");
        if !(mins > 0.0 && self.counting() && self.has("exp_region") && self.exp_last.is_some()) {
            return;
        }
        let idle = self.now() - self.exp_move_t.max(self.run_start);
        if idle < mins * 60.0 {
            return;
        }
        self.toggle_run(idle);
        self.auto_paused = true;
        let m = fmt_g(mins);
        self.log(format!("{m} 分鐘沒有經驗，自動暫停（最後一次有經驗之後的 {} 不算）", dur(Some(idle))));
        self.say(format!("{m} 分鐘沒有經驗，自動暫停，這段時間不算。打到怪會自動繼續"), 60.0);
    }

    fn exp_moved(&mut self, before: (i64, Option<f64>)) {
        self.exp_move_t = self.now();
        if !(self.auto_paused && self.running && self.paused) {
            return;
        }
        let d = self.exp_last.unwrap().0 - before.0;
        self.toggle_run(0.0);
        if d > 0 {
            self.gain += d;
            let a = self.active();
            self.hist.push((a, self.gain));
        }
        let msg = self.alert.as_ref().map_or("繼續計時".to_string(), |a| a.0.clone());
        self.log("打到怪了，自動繼續計時".into());
        self.say(format!("打到怪了，自動{msg}"), 8.0);
    }

    pub fn ask_reset(&mut self) {
        let now = self.now();
        if now - self.reset_armed < 3.0 {
            self.reset_armed = 0.0;
            let saved = self.log_session();
            if saved == Saved::Failed {
                self.say("寫不進練功紀錄（Excel 開著嗎？），這次沒有清除。關掉檔案再按兩下 ⟲", 10.0);
                return;
            }
            let s = self.stats();
            let p = s.profit.map_or("–".to_string(), py_num);
            self.log(format!("===== 結束這一段：EXP {}、收益 {p} =====", comma(self.gain)));
            self.running = false;
            self.paused = false;
            self.new_session();
            for k in self.ks() {
                if !self.is_qs_mode(k) {
                    self.total[k] = None;
                }
            }
            self.save_state();
            match saved {
                Saved::TooShort => self.say("已清除（不到 1 分鐘，沒有存）", 4.0),
                Saved::Main => self.say("已清除，本次成績存進 練功紀錄.csv", 4.0),
                Saved::Other(p) => self.say(format!("練功紀錄.csv 被占用，改存到 {p}"), 8.0),
                Saved::Failed => {}
            }
        } else {
            self.reset_armed = now;
            self.say("3 秒內再按一次 ⟲ 就清除本次紀錄", 3.0);
        }
    }

    pub fn on_ocr(&mut self, out: &OcrOut) {
        self.ocr_seen = self.now();
        let mut parts: Vec<String> = vec![];
        let mut game_visible = true;
        let before = self.exp_last;
        if let Some(text) = &out.exp {
            self.raw.insert("exp".into(), text.clone());
            let p = parse_exp(text);
            if p.is_some() {
                self.exp_ok_t = self.now();
            }
            game_visible = p.is_some() || self.now() - self.exp_ok_t > 1800.0;
            if out.exp_err.is_some() {
                parts.push("EXP 截圖失敗（螢幕鎖定或被擋住）".into());
            } else if let Some((e, pct)) = p {
                let ok = self.feed_exp(e, Some(pct), false);
                parts.push(if ok { "EXP ✓" } else { "EXP 確認中" }.into());
            } else {
                parts.push("EXP 讀不到".into());
            }
            if let (Some(b), Some(l)) = (before, self.exp_last) {
                if l.0 != b.0 {
                    self.exp_moved(b);
                }
            }
        }
        let mut qs_raw: [Option<i64>; NP] = [None; NP];
        for k in self.ks() {
            let Some(text) = &out.qs[k] else { continue };
            self.raw.insert(KS[k].into(), text.clone());
            let cap = self.cfg_f("stack_max") as i64;
            let v = parse_count(text).filter(|&x| !(cap != 0 && x > cap));
            qs_raw[k] = v.filter(|_| out.qs_err[k].is_none());
            if out.qs_err[k].is_some() {
                self.qs_blind_t[k] = self.now();
                parts.push(format!("{} 截圖失敗", util::short_name(&self.name(k))));
            } else if v.is_none() {
                self.qs_blind_t[k] = self.now();
                parts.push(format!("{} 讀不到", util::short_name(&self.name(k))));
            } else if self.qs_lost.0.is_some() {
                self.qs_blind_t[k] = self.now();
                parts.push(format!("{} 讀不到", util::short_name(&self.name(k))));
            } else if game_visible {
                self.feed_qs(k, v.unwrap());
            } else {
                self.qs_blind_t[k] = self.now();
            }
            if cap != 0 && self.counting() && self.qs[k] == Some(cap) {
                self.cap_since[k] = Some(self.cap_since[k].unwrap_or_else(|| self.now()));
            } else {
                self.cap_since[k] = None;
            }
        }
        let steady: Vec<bool> = (0..NP)
            .map(|k| match qs_raw[k] {
                Some(v) if v.to_string().bytes().any(|c| c > b'1') => {
                    self.qs_prev_raw[k] == Some(v) || self.qs[k].map_or(false, |q| v < q && q - v <= 15)
                }
                _ => false,
            })
            .collect();
        for k in self.ks() {
            if out.qs[k].is_some() {
                self.qs_prev_raw[k] = qs_raw[k];
            }
        }
        let framed: Vec<usize> = (0..NP).filter(|&k| out.qs[k].is_some()).collect();
        let both_lost = framed.len() >= 2 && framed.iter().all(|&k| out.qs_err[k].is_none() && !steady[k]);
        if both_lost && game_visible && out.exp.is_some() {
            let since = *self.qs_lost.0.get_or_insert(self.now());
            if !self.qs_lost.1 && self.now() - since >= 15.0 {
                self.qs_lost.1 = true;
                let msg = "快捷欄讀取失敗，打開快捷欄或按 ☰ 檢查設定";
                self.say(msg, 15.0);
                self.log(msg.into());
            }
        } else if !both_lost {
            self.qs_lost = (None, false);
        }
        let mut results: Vec<&'static str> = vec![];
        if let Some(inv) = &out.inv {
            let mut info: Vec<String> = vec![];
            let bag_open = inv.iter().any(|x| x.as_ref().map_or(false, |v| !v.is_empty()));
            self.both_drop = (0..NP)
                .filter(|&k| {
                    inv[k].as_ref().map_or(false, |s| !s.is_empty() && self.inv_slots[k].map_or(false, |sl| (s.len() as i64) < sl))
                        && self.total[k].map_or(false, |t| self.now() - t.3 < 30.0)
                })
                .count()
                >= 2;
            for k in self.ks() {
                let name = self.name(k);
                let Some(stacks) = &inv[k] else {
                    if bag_open && self.has("inv_region") && !self.inv_ready(Some(k)) {
                        info.push(format!("{name} 還沒框背包裡的一格，讀不到"));
                    }
                    continue;
                };
                if stacks.is_empty() {
                    let qs_has = self.region(k) && self.qs[k].unwrap_or(0) > 0;
                    if bag_open && !qs_has && self.total[k].is_some() {
                        let r = self.feed_inv(k, &[]);
                        results.push(r);
                        info.push(format!("{name} 沒看到，當成用完了{}", if r == "wait" { "（確認中）" } else { "" }));
                    } else {
                        info.push(format!("{name} 沒看到"));
                    }
                    continue;
                }
                self.check_qs_bag(k, stacks, qs_raw[k], out.bag_id);
                let r = self.feed_inv(k, stacks);
                results.push(r);
                if r == "bad" {
                    info.push(format!("{name} {}格（有格子數字讀不到，一直這樣就重框「背包裡一格{name}」）", stacks.len()));
                } else {
                    let sum: i64 = stacks.iter().map(|v| v.unwrap_or(0)).sum();
                    info.push(format!("{name} {}格 共{}{}", stacks.len(), comma(sum), if r == "wait" { "（確認中）" } else { "" }));
                }
            }
            self.inv_msg = info.join("｜");
        }
        if let Some(text) = &out.meso {
            self.raw.insert("meso".into(), text.clone());
            if let Some(v) = parse_meso(text) {
                if out.meso_box_only && !self.meso_near(v) && self.meso_settled(v) {
                    self.log(format!("楓幣一直讀到 {} 左右，跟上次差很多但很穩定，當作錢真的變了", comma(v)));
                    self.meso_pend = Some((v, 9, BTreeSet::from([-1])));
                    let r = self.feed_meso(v, out.bag_id);
                    results.push(r);
                } else if out.meso_box_only && !self.meso_near(v) {
                    if self.meso_rej != Some(v) {
                        self.meso_rej = Some(v);
                        let tail = self.meso.map_or("還沒讀過楓幣".to_string(), |l| format!("跟上次的 {} 差太多", comma(l)));
                        self.log(format!("楓幣讀到 {}，{tail}，先不採用（一直讀到差不多的數字、或背包裡看到藥水時再確認）", comma(v)));
                    }
                } else {
                    self.meso_cand = None;
                    let r = self.feed_meso(v, out.bag_id);
                    results.push(r);
                }
            }
        }
        if !results.is_empty() {
            let ok = results.contains(&"ok") && !results.contains(&"wait");
            parts.push(if ok { "背包 ✓" } else { "背包讀取中…請保持打開" }.into());
        }
        self.ocr_msg = format!("自動讀取：{}", if parts.is_empty() { "OK".to_string() } else { parts.join("、") });
    }

    pub fn submit_record(&mut self, exp: Option<i64>, pct: Option<f64>, pots: [Option<i64>; 2]) -> Result<(), String> {
        if let (Some(e), None, Some(l)) = (exp, pct, self.exp_last) {
            if e < l.0 {
                return Err(format!("EXP 比上次（{}）少，是升級了嗎？請一起填 EXP %", comma(l.0)));
            }
        }
        if let Some(e) = exp {
            let mut pct = pct;
            if pct.is_none() {
                if let Some(n) = self.need.filter(|&n| n != 0.0) {
                    if !self.exp_last.map_or(false, |l| e < l.0) {
                        pct = Some(e as f64 * 100.0 / n);
                    }
                }
            }
            self.feed_exp(e, pct, true);
        }
        for (k, v) in pots.iter().enumerate() {
            if let Some(v) = v {
                self.set_total(k, *v, None);
            }
        }
        Ok(())
    }

    fn get_json(&self, key: &str) -> Value {
        let per = |f: &dyn Fn(usize) -> Value| Value::Object((0..NP).map(|k| (KS[k].to_string(), f(k))).collect());
        match key {
            "gain" => json!(self.gain),
            "hist" => json!(self.hist.iter().map(|h| json!([h.0, h.1])).collect::<Vec<_>>()),
            "levelups" => json!(self.levelups),
            "deaths" => json!(self.deaths),
            "qs_used" => per(&|k| json!(self.qs_used[k])),
            "bag_used" => per(&|k| json!(self.bag_used[k])),
            "meso_anchor" => self.meso_anchor.map_or(Value::Null, |m| json!([m.0, m.1, m.2])),
            "series" => json!(self.series.iter().map(|s| json!([s.0, s.1, s.2, s.3, s.4, s.5])).collect::<Vec<_>>()),
            "curve" => curve_json(&self.curve),
            "qs_sus_cnt" => per(&|k| json!(self.qs_sus_cnt[k])),
            "cost_prev" => per(&|k| json!(self.cost_prev[k])),
            "prev_price" => per(&|k| json!(self.prev_price[k])),
            "pot_names" => per(&|k| json!(self.pot_names[k])),
            "used_mark" => per(&|k| json!(self.used_mark[k])),
            "last_drop" => per(&|k| self.last_drop[k].as_ref().map_or(Value::Null, |d| json!([d.miss, d.t, d.n]))),
            "anchor_used" => per(&|k| json!(self.anchor_used[k])),
            "inc_done" => self.inc_done.map_or(Value::Null, |d| json!([d.0, d.1])),
            "seg_pending" => json!(self.seg_pending),
            "sold" => json!(self.sold),
            "pause_up" => self.pause_up.map_or(Value::Null, |p| json!([p.0, p.1])),
            "sale_ask" => json!(self.sale_ask),
            "recon_acc" => per(&|k| json!(self.recon_acc[k])),
            "recon_add" => per(&|k| json!(self.recon_add[k])),
            "need" => json!(self.need),
            "qs_all" => per(&|k| json!(self.qs_all[k])),
            "qs_add" => per(&|k| json!(self.qs_add[k])),
            "total" => per(&|k| self.total[k].map_or(Value::Null, |t| json!([t.0, t.1, t.2, t.3]))),
            "total_act" => per(&|k| json!(self.total_act[k])),
            "inv_slots" => per(&|k| json!(self.inv_slots[k])),
            "last_rise" => per(&|k| self.last_rise[k].map_or(Value::Null, |r| json!([r.0, r.1, r.2, r.3, r.4]))),
            "meso" => json!(self.meso),
            "meso_t" => json!(self.meso_t),
            "qs_sus" => per(&|k| json!(self.qs_sus[k])),
            _ => Value::Null,
        }
    }

    pub fn state_json(&self) -> Value {
        let mut m = Map::new();
        for k in SESSION_KEYS.iter().chain(KEEP_KEYS.iter()) {
            m.insert(k.to_string(), self.get_json(k));
        }
        let a = self.active();
        m.insert("hist".into(), json!(self.hist.iter().filter(|x| x.0 > a - 3900.0).map(|h| json!([h.0, h.1])).collect::<Vec<_>>()));
        m.insert(
            "series".into(),
            json!(self.series.iter().filter(|x| x.0 > a - 3900.0).map(|s| json!([s.0, s.1, s.2, s.3, s.4, s.5])).collect::<Vec<_>>()),
        );
        m.insert("saved_at".into(), json!(self.now()));
        m.insert("acc".into(), json!(a));
        m.insert("running".into(), json!(self.running));
        m.insert("started_at".into(), json!(self.started_at));
        Value::Object(m)
    }

    pub fn save_state(&mut self) -> bool {
        let Some(path) = self.state_path.clone() else { return true };
        let tmp = path.with_extension("json.tmp");
        let data = serde_json::to_string(&self.state_json()).unwrap();
        std::fs::write(&tmp, data).and_then(|_| std::fs::rename(&tmp, &path)).is_ok()
    }

    fn set_json(&mut self, key: &str, v: &Value) {
        let i = |v: &Value| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64));
        let f = |v: &Value| v.as_f64();
        match key {
            "gain" => self.gain = i(v).unwrap_or(0),
            "hist" => self.hist = v.as_array().map_or(vec![], |a| a.iter().filter_map(|x| Some((f(&x[0])?, i(&x[1])?))).collect()),
            "levelups" => self.levelups = i(v).unwrap_or(0),
            "deaths" => self.deaths = i(v).unwrap_or(0),
            "series" => {
                self.series = v.as_array().map_or(vec![], |a| {
                    a.iter()
                        .filter_map(|x| {
                            let mut u: [i64; NP] = std::array::from_fn(|k| x.get(5).and_then(|v| v.get(k)).and_then(i).unwrap_or(0));
                            let (h, m) = (i(&x[2])?, i(&x[3])?);
                            (u[0], u[1]) = (h, m);
                            Some((f(&x[0])?, i(&x[1])?, h, m, i(&x[4]), u))
                        })
                        .collect()
                })
            }
            "curve" => self.curve = parse_curve(v),
            "seg_pending" => self.seg_pending = v.as_bool().unwrap_or(false),
            "sold" => self.sold = i(v).unwrap_or(0),
            "pause_up" => self.pause_up = v.as_array().and_then(|a| Some((i(a.first()?)?, i(a.get(1)?)?))),
            "sale_ask" => self.sale_ask = i(v),
            "need" => self.need = f(v).filter(|&n| exp_table::level_of(n).is_some()),
            "meso" => self.meso = i(v),
            "meso_t" => self.meso_t = f(v).unwrap_or(0.0),
            "inc_done" => self.inc_done = v.as_array().map(|a| (i(&a[0]).unwrap_or(0), a.last().and_then(f).unwrap_or(0.0))),
            _ => {
                for k in 0..NP {
                    let x = v.get(KS[k]).cloned().unwrap_or(Value::Null);
                    match key {
                        "qs_used" => self.qs_used[k] = i(&x).unwrap_or(0),
                        "bag_used" => self.bag_used[k] = i(&x).unwrap_or(0),
                        "qs_sus_cnt" => self.qs_sus_cnt[k] = i(&x).unwrap_or(0),
                        "cost_prev" => self.cost_prev[k] = f(&x).unwrap_or(0.0),
                        "prev_price" => self.prev_price[k] = f(&x),
                        "pot_names" => {
                            self.pot_names[k] = x.as_array().map_or(vec![], |a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                        }
                        "used_mark" => self.used_mark[k] = i(&x).unwrap_or(0),
                        "recon_acc" => self.recon_acc[k] = i(&x).unwrap_or(0),
                        "recon_add" => self.recon_add[k] = i(&x).unwrap_or(0),
                        "qs_all" => self.qs_all[k] = i(&x).unwrap_or(0),
                        "qs_add" => self.qs_add[k] = i(&x).unwrap_or(0),
                        "qs_sus" => self.qs_sus[k] = i(&x).unwrap_or(0),
                        "total_act" => self.total_act[k] = f(&x).unwrap_or(0.0),
                        "inv_slots" => self.inv_slots[k] = i(&x),
                        "total" => self.total[k] = x.as_array().and_then(|a| Some((i(a.first()?)?, i(a.get(1)?)?, i(a.get(2)?)?, f(a.get(3)?)?))),
                        "last_rise" => {
                            self.last_rise[k] = x
                                .as_array()
                                .filter(|a| a.len() >= 5)
                                .and_then(|a| Some((i(&a[0])?, i(&a[1])?, f(&a[2])?, i(&a[3])?, i(&a[4])?)))
                        }
                        "last_drop" => {
                            self.last_drop[k] = x.as_array().and_then(|a| Some(Drop { miss: i(a.first()?)?, t: f(a.get(1)?)?, n: i(a.get(2)?)? }))
                        }
                        "anchor_used" => self.anchor_used[k] = i(&x).unwrap_or(0),
                        _ => {}
                    }
                }
            }
        }
    }

    pub fn load_state(&mut self, data: &Value) {
        for k in KEEP_KEYS {
            if let Some(v) = data.get(k) {
                self.set_json(k, v);
            }
        }
        if !data.get("running").and_then(|v| v.as_bool()).unwrap_or(false) {
            return;
        }
        for k in SESSION_KEYS {
            if let Some(v) = data.get(k) {
                self.set_json(k, v);
            }
        }
        let ma = data.get("meso_anchor").and_then(|v| v.as_array()).cloned();
        let d = data.get("inc_done").and_then(|v| v.as_array()).cloned();
        let old = ma.as_ref().map_or(false, |a| a.len() != 3) || d.as_ref().map_or(false, |a| a.len() != 2);
        self.meso_anchor = ma.and_then(|a| {
            let g = |i: usize| a.get(i).cloned().unwrap_or(Value::Null);
            let (m, act, t) = match a.len() {
                2 => (g(0), json!(0.0), Value::Null),
                4 => (g(0), g(2), g(3)),
                _ => (g(0), g(1), g(2)),
            };
            let t = if t.is_f64() { t.as_f64().filter(|&x| x > 1e9) } else { None };
            Some((m.as_i64()?, act.as_f64().unwrap_or(0.0), t))
        });
        if old {
            self.series = vec![];
        }
        self.acc = data.get("acc").and_then(|v| v.as_f64()).unwrap_or(0.0);
        self.started_at = data.get("started_at").and_then(|v| v.as_str()).map(|s| s.to_string());
        self.running = true;
        self.paused = true;
        self.run_start = 0.0;
        let away = (self.now() - data.get("saved_at").and_then(|v| v.as_f64()).unwrap_or(0.0)) / 3600.0;
        if away > RESUME_HOURS {
            let saved = self.log_session();
            self.log(format!("上一段（{}）隔了 {:.1} 小時才開，存進練功紀錄後重新開始", hms(self.acc), away));
            self.running = false;
            self.paused = false;
            self.new_session();
            for k in self.ks() {
                if !self.is_qs_mode(k) {
                    self.total[k] = None;
                }
            }
            self.save_state();
            let ok = !matches!(saved, Saved::Failed | Saved::TooShort);
            self.say(if ok { "上一段已經存進練功紀錄.csv，這次重新開始" } else { "上一段太短沒有存，這次重新開始" }, 8.0);
        } else {
            if !self.seg_pending {
                let a = self.acc;
                self.close_seg(a);
            }
            self.log(format!("接續上一段（已練 {}，暫停中）", hms(self.acc)));
            self.say(format!("已接回上一段（{}），暫停中，按 ▶ 繼續", hms(self.acc)), 10.0);
        }
    }

    pub fn csv_header(&self) -> Vec<String> {
        [
            "開始時間".to_string(),
            "練功分鐘".into(),
            "獲得EXP".into(),
            "EXP每小時".into(),
            "升級次數".into(),
            "死亡次數".into(),
            self.name(0) + "用量",
            self.name(1) + "用量",
            "藥水花費".into(),
            "花費每小時".into(),
            "每萬楓幣EXP".into(),
            "撿到楓幣".into(),
            "淨賺楓幣".into(),
            "淨賺每小時".into(),
            "賣掉落物".into(),
            "紅水名稱".into(),
            "藍水名稱".into(),
            "其他藥水".into(),
        ]
        .to_vec()
    }

    pub fn log_session(&mut self) -> Saved {
        self.flush_drift();
        let a = self.active();
        if a < 60.0 {
            return Saved::TooShort;
        }
        let Some(path) = self.log_path.clone() else { return Saved::Main };
        let s = self.stats();
        let header = self.csv_header();
        let started = self.started_at.clone().unwrap_or_else(local_iso);
        let row = vec![
            started.get(..16).unwrap_or(&started).replace('T', " "),
            py_float((a / 60.0 * 10.0).round_ties_even() / 10.0),
            self.gain.to_string(),
            pround(s.rate.unwrap_or(0.0)).to_string(),
            self.levelups.to_string(),
            self.deaths.to_string(),
            self.used(0).to_string(),
            self.used(1).to_string(),
            py_num(s.cost),
            pround(s.cost_hr.unwrap_or(0.0)).to_string(),
            pround(s.eff.unwrap_or(0.0)).to_string(),
            s.income.map_or(String::new(), |v| v.to_string()),
            s.profit.map_or(String::new(), py_num),
            s.profit_hr.map_or(String::new(), |v| pround(v).to_string()),
            if self.sold > 0 { self.sold.to_string() } else { String::new() },
            self.pot_label(0),
            self.pot_label(1),
            self.extra_label(),
        ];
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let ext = path.extension().map_or("csv".to_string(), |e| e.to_string_lossy().to_string());
        let alt = path.with_file_name(format!("{stem}_{}.{ext}", local_stamp()));
        for (i, p) in [path, alt].iter().enumerate() {
            if write_csv_row(p, &header, &row).is_ok() {
                let line = json!({"start": row[0], "pts": curve_json(&self.curve_now())}).to_string() + "\n";
                let _ = append_line(&p.with_file_name(CURVE_FILE), &line);
                return if i == 0 { Saved::Main } else { Saved::Other(p.file_name().unwrap().to_string_lossy().into()) };
            }
        }
        Saved::Failed
    }
}

pub fn remove_csv_lines(path: &std::path::Path, pick: &dyn Fn(&str) -> bool) -> std::io::Result<Vec<String>> {
    let raw = std::fs::read(path)?;
    let bom = raw.starts_with(b"\xef\xbb\xbf");
    let text = String::from_utf8_lossy(if bom { &raw[3..] } else { &raw }).to_string();
    let (gone, keep): (Vec<&str>, Vec<&str>) = text.split_inclusive('\n').partition(|l| pick(l));
    if gone.is_empty() {
        return Ok(vec![]);
    }
    let mut out = if bom { b"\xef\xbb\xbf".to_vec() } else { vec![] };
    for l in keep {
        out.extend(l.as_bytes());
    }
    let tmp = path.with_extension("csv.tmp");
    std::fs::write(&tmp, out)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(gone.into_iter().map(|l| l.to_string()).collect())
}

pub fn restore_csv_lines(path: &std::path::Path, header: &[String], lines: &[String]) -> std::io::Result<()> {
    use std::io::Write;
    if !path.exists() {
        let head = header.iter().map(|s| csv_field(s)).collect::<Vec<_>>().join(",") + "\r\n";
        let mut out = b"\xef\xbb\xbf".to_vec();
        out.extend(head.as_bytes());
        std::fs::write(path, out)?;
    }
    let mut f = std::fs::OpenOptions::new().append(true).open(path)?;
    let raw = std::fs::read(path)?;
    if !raw.is_empty() && !raw.ends_with(b"\n") {
        f.write_all(b"\r\n")?;
    }
    for l in lines {
        f.write_all(l.as_bytes())?;
        if !l.ends_with('\n') {
            f.write_all(b"\r\n")?;
        }
    }
    Ok(())
}

pub fn curve_json(v: &[CurvePt]) -> Value {
    json!(v.iter().map(|c| json!([(c.0 * 10.0).round() / 10.0, c.1, c.2, c.3, c.4, c.5.round(), c.6])).collect::<Vec<_>>())
}

pub fn parse_curve(v: &Value) -> Vec<CurvePt> {
    let f = |x: &Value| x.as_f64();
    let i = |x: &Value| x.as_i64().or_else(|| x.as_f64().map(|v| v.round() as i64));
    v.as_array().map_or(vec![], |a| a.iter().filter_map(|x| Some((f(&x[0])?, i(&x[1])?, i(&x[2])?, i(&x[3])?, i(&x[4]), f(&x[5]).unwrap_or(0.0), i(&x[6]).unwrap_or(0)))).collect())
}

fn append_line(p: &std::path::Path, line: &str) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::OpenOptions::new().create(true).append(true).open(p)?.write_all(line.as_bytes())
}

fn py_opt(v: Option<f64>) -> String {
    v.map_or("None".into(), py_float)
}

pub fn py_float(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e16 {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

pub fn py_num(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e16 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn digits_differ(a: i64, b: i64) -> usize {
    let (a, b) = (a.to_string().into_bytes(), b.to_string().into_bytes());
    let at = |s: &[u8], i: usize| s.len().checked_sub(i + 1).map(|j| s[j]);
    (0..a.len().max(b.len())).filter(|&i| at(&a, i) != at(&b, i)).count()
}

fn write_csv_row(path: &std::path::Path, header: &[String], row: &[String]) -> std::io::Result<()> {
    use std::io::Write;
    let line = |v: &[String]| v.iter().map(|s| csv_field(s)).collect::<Vec<_>>().join(",") + "\r\n";
    if path.exists() {
        let raw = std::fs::read(path)?;
        if let Ok(text) = std::str::from_utf8(raw.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&raw)) {
            let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
            if lines.first().map_or(false, |f| f.trim_end().split(',').count() < header.len()) {
                lines.remove(0);
                let mut out = b"\xef\xbb\xbf".to_vec();
                out.extend(line(header).as_bytes());
                for l in lines {
                    out.extend(l.as_bytes());
                }
                std::fs::write(path, out)?;
            }
        }
        let mut f = std::fs::OpenOptions::new().append(true).open(path)?;
        f.write_all(line(row).as_bytes())
    } else {
        let mut out = b"\xef\xbb\xbf".to_vec();
        out.extend(line(header).as_bytes());
        out.extend(line(row).as_bytes());
        std::fs::write(path, out)
    }
}

pub fn local_iso() -> String {
    let (y, mo, d, h, mi, s, us) = local_now();
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{us:06}")
}

fn local_stamp() -> String {
    let (y, mo, d, h, mi, s, _) = local_now();
    format!("{y:04}{mo:02}{d:02}_{h:02}{mi:02}{s:02}")
}

pub fn log_stamp() -> String {
    let (_, mo, d, h, mi, s, _) = local_now();
    format!("{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

pub fn local_now() -> (i64, i64, i64, i64, i64, i64, i64) {
    let now = sys_now();
    let us = ((now.fract()) * 1e6) as i64;
    let secs = now as i64 + local_offset_secs();
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d, rem / 3600, rem / 60 % 60, rem % 60, us)
}

#[cfg(windows)]
fn local_offset_secs() -> i64 {
    #[repr(C)]
    struct Tzi {
        bias: i32,
        _name: [u16; 32],
        _sd: [u16; 8],
        standard_bias: i32,
        _dn: [u16; 32],
        _dd: [u16; 8],
        daylight_bias: i32,
    }
    unsafe extern "system" {
        fn GetTimeZoneInformation(tz: *mut Tzi) -> u32;
    }
    unsafe {
        let mut tz: Tzi = std::mem::zeroed();
        let r = GetTimeZoneInformation(&mut tz);
        let bias = tz.bias + if r == 2 { tz.daylight_bias } else { tz.standard_bias };
        -(bias as i64) * 60
    }
}

#[cfg(not(windows))]
fn local_offset_secs() -> i64 {
    #[repr(C)]
    struct Tm {
        _f: [i32; 9],
        gmtoff: i64,
        _zone: *const u8,
    }
    unsafe extern "C" {
        fn time(t: *mut i64) -> i64;
        fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
    }
    unsafe {
        let t = time(std::ptr::null_mut());
        let mut tm: Tm = std::mem::zeroed();
        localtime_r(&t, &mut tm);
        tm.gmtoff
    }
}

fn drop_one_digit(val: i64, last: i64, step: f64) -> Option<i64> {
    let s = val.to_string();
    if s.len() != last.to_string().len() + 1 {
        return None;
    }
    (0..s.len())
        .filter_map(|i| {
            let mut t = s.clone();
            t.remove(i);
            t.parse::<i64>().ok()
        })
        .filter(|&c| c <= last && (last - c) as f64 <= step)
        .max()
}

#[cfg(test)]
mod switch_tests {
    use super::*;

    #[test]
    fn meso_seen_without_potions_must_be_close() {
        let mut t = Tracker::new(default_cfg(), Some(1.8e9));
        t.meso = Some(112_650_109);
        let out = |m: &str| OcrOut { meso: Some(m.into()), meso_box_only: true, ..Default::default() };
        for _ in 0..5 {
            t.on_ocr(&out("172,650,109"));
        }
        assert_eq!(t.meso, Some(112_650_109));
        assert!(t.meso_pend.is_none());
        for _ in 0..5 {
            t.on_ocr(&out("112,700,000"));
        }
        assert_eq!(t.meso, Some(112_700_000));
        let mut t2 = Tracker::new(default_cfg(), Some(1.8e9));
        for _ in 0..5 {
            t2.on_ocr(&out("112,700,000"));
        }
        assert_eq!(t2.meso, None);
    }

    #[test]
    fn steady_new_meso_is_accepted_but_one_digit_misread_is_not() {
        let mut t = Tracker::new(default_cfg(), Some(1.8e9));
        t.meso = Some(111_779_054);
        let out = |m: &str| OcrOut { meso: Some(m.into()), meso_box_only: true, ..Default::default() };
        for i in 0..20 {
            t.fake_now = Some(1.8e9 + i as f64 * 2.0);
            t.on_ocr(&out("171,779,054"));
        }
        assert_eq!(t.meso, Some(111_779_054));
        let mut seen = vec![];
        for i in 0..10 {
            t.fake_now = Some(1.9e9 + i as f64 * 2.0);
            t.on_ocr(&out(&format!("89,27{i},000")));
            seen.push(t.meso);
        }
        assert_eq!(seen[3], Some(111_779_054));
        assert_eq!(seen[5], Some(89_275_000));
    }

    #[test]
    fn meso_gained_while_paused_is_asked_as_sale() {
        let mut t = Tracker::new(default_cfg(), Some(1.8e9));
        t.cfg.insert("meso_region".into(), json!([1, 1, 10, 10]));
        let bag = |t: &mut Tracker, m: i64, at: f64| {
            for j in 0..3 {
                t.fake_now = Some(at + j as f64);
                t.on_ocr(&OcrOut { meso: Some(comma(m)), ..Default::default() });
            }
        };
        t.toggle_run(0.0);
        bag(&mut t, 50_000_000, 1.8e9 + 1.0);
        bag(&mut t, 50_400_000, 1.8e9 + 600.0);
        assert_eq!(t.income(), Some(400_000));
        t.toggle_run(0.0);
        bag(&mut t, 58_400_000, 1.8e9 + 700.0);
        bag(&mut t, 55_400_000, 1.8e9 + 800.0);
        assert_eq!(t.sale_ask, None);
        t.fake_now = Some(1.8e9 + 890.0);
        t.toggle_run(0.0);
        assert_eq!(t.sale_ask, None, "繼續時背包沒開著：等讀到楓幣再問");
        bag(&mut t, 55_400_000, 1.8e9 + 900.0);
        assert_eq!(t.sale_ask, Some(8_000_000));
        t.answer_sale(true);
        assert_eq!((t.sale_ask, t.sold), (None, 8_000_000));
        bag(&mut t, 55_500_000, 1.8e9 + 1000.0);
        let s = t.stats();
        assert_eq!(s.income, Some(500_000));
        assert_eq!(s.profit, Some(8_500_000.0));
        assert_eq!(t.curve_pt(1.0).6, 8_000_000);
        let saved = t.state_json();
        let mut u = Tracker::new(default_cfg(), Some(1.8e9 + 1000.0));
        u.load_state(&saved);
        assert_eq!(u.sold, 8_000_000);
        let mut t = Tracker::new(default_cfg(), Some(1.8e9));
        t.cfg.insert("meso_region".into(), json!([1, 1, 10, 10]));
        t.toggle_run(0.0);
        bag(&mut t, 50_000_000, 1.8e9 + 1.0);
        t.toggle_run(0.0);
        bag(&mut t, 53_000_000, 1.8e9 + 100.0);
        t.toggle_run(0.0);
        assert_eq!(t.sale_ask, Some(3_000_000));
        t.answer_sale(false);
        assert_eq!((t.sold, t.stats().profit), (0, Some(0.0)));
    }

    #[test]
    fn switching_keeps_old_price_for_used_potions() {
        let mut t = Tracker::new(default_cfg(), Some(1.8e9));
        t.cfg.insert("hp_name".into(), json!("沙嗲"));
        t.cfg.insert("hp_price".into(), json!(2600));
        t.qs_used[0] = 10;
        assert_eq!(t.cost_of(0), 26000.0);
        t.switch_potion(0, "馴鹿奶", 5600.0, None);
        assert_eq!(t.cost_of(0), 26000.0);
        t.qs_used[0] = 13;
        assert_eq!(t.cost_of(0), 26000.0 + 3.0 * 5600.0);
        assert_eq!(t.span_cost(0, 8, 13), 2.0 * 2600.0 + 3.0 * 5600.0);
        assert_eq!(t.span_cost(0, 11, 13), 2.0 * 5600.0);
        assert_eq!((t.pot_label(0), t.pot_label(1)), ("沙嗲→馴鹿奶".to_string(), "紅豆刨冰".to_string()));
        t.switch_potion(1, "藍藥水", 200.0, None);
        assert_eq!(t.pot_label(1), "藍藥水");
        let data = { t.running = true; t.state_json() };
        let mut u = Tracker::new(t.cfg.clone(), Some(1.8e9));
        u.load_state(&data);
        assert_eq!(u.pot_label(0), "沙嗲→馴鹿奶");
        assert_eq!(&t.csv_header()[15..], ["紅水名稱", "藍水名稱", "其他藥水"]);
        assert_eq!(t.ks(), vec![0, 1]);
        t.cfg.insert("p3_name".into(), json!("力量藥水"));
        t.cfg.insert("p3_price".into(), json!(1000));
        assert_eq!(t.ks(), vec![0, 1, 2]);
        let before = t.cost_all();
        t.qs_used[2] = 3;
        assert_eq!(t.cost_all(), before + 3000.0);
        assert_eq!(t.extra_label(), "力量藥水 3");
        let data = { t.running = true; t.state_json() };
        let mut v = Tracker::new(t.cfg.clone(), Some(1.8e9));
        v.load_state(&data);
        assert_eq!(v.used(2), 3);
        let mut old = data.clone();
        old["qs_used"] = json!({"hp": 13, "mp": 0});
        let mut w = Tracker::new(t.cfg.clone(), Some(1.8e9));
        w.load_state(&old);
        assert_eq!((w.used(0), w.used(2)), (13, 0));
        let cost = t.cost_all();
        t.switch_potion(2, "", 1000.0, None);
        assert_eq!(t.ks(), vec![0, 1]);
        assert_eq!(t.cost_all(), cost);
        assert_eq!(t.extra_label(), "力量藥水 3");
        t.switch_potion(2, "經驗加倍", 500.0, None);
        t.qs_used[2] = 5;
        assert_eq!(t.cost_all(), cost + 2.0 * 500.0);
        assert_eq!(t.extra_label(), "力量藥水→經驗加倍 5");
        let mut t2 = Tracker::new(default_cfg(), Some(1.8e9));
        t2.cfg.insert("hp_price".into(), json!(2600));
        t2.qs_used[0] = 13;
        t2.switch_potion(0, "馴鹿奶", 5600.0, Some(11));
        assert_eq!(t2.cost_of(0), 11.0 * 2600.0 + 2.0 * 5600.0);
        t2.running = true;
        let data = t2.state_json();
        let mut t3 = Tracker::new(t2.cfg.clone(), Some(1.8e9));
        t3.load_state(&data);
        assert_eq!(t3.cost_of(0), t2.cost_of(0));
        t3.new_session();
        assert_eq!(t3.cost_of(0), 0.0);
    }
}

#[cfg(test)]
mod exp_tests {
    use super::*;

    fn read(t: &mut Tracker, exp: i64, pct: f64) {
        let out = OcrOut { exp: Some(format!("{exp}[{pct:.2}%]")), ..Default::default() };
        t.on_ocr(&out);
        t.fake_now = Some(t.now() + 3.0);
    }

    #[test]
    fn level_up_uses_the_table() {
        let mut t = Tracker::new(default_cfg(), Some(1.8e9));
        t.cfg.insert("exp_region".into(), json!([0, 0, 10, 10]));
        t.running = true;
        let need152 = exp_table::need(152).unwrap();
        let need153 = exp_table::need(153).unwrap();
        for e in [163_000_000i64, 163_000_000, 163_400_000, 163_836_876] {
            read(&mut t, e, (e as f64 * 100.0 / need152 as f64 * 100.0).floor() / 100.0);
        }
        assert_eq!(t.level(), Some(152));
        assert_eq!(t.need, Some(need152 as f64));
        read(&mut t, 39_943, 0.02);
        read(&mut t, 39_943, 0.02);
        assert_eq!(t.levelups, 1);
        assert_eq!(t.level(), Some(153));
        assert_eq!(t.need, Some(need153 as f64));
        let mut e = 39_943i64;
        let gain0 = t.gain;
        let mut got = 0;
        while e < 6_000_000 {
            e += 140_000;
            got += 140_000;
            read(&mut t, e, (e as f64 * 100.0 / need153 as f64 * 100.0).floor() / 100.0);
        }
        assert_eq!(t.gain - gain0, got, "{:#?}", t.logs);
        assert!(!t.logs.iter().any(|l| l.contains("對不上")), "{:#?}", t.logs);
    }

    #[test]
    fn garbage_exp_read_is_not_a_new_base() {
        let mut t = Tracker::new(default_cfg(), Some(1.8e9));
        t.cfg.insert("exp_region".into(), json!([0, 0, 10, 10]));
        t.running = true;
        let need153 = exp_table::need(153).unwrap();
        let e = 44_703_895i64;
        let p = (e as f64 * 100.0 / need153 as f64 * 100.0).floor() / 100.0;
        read(&mut t, e, p);
        read(&mut t, e, p);
        assert_eq!(t.level(), Some(153));
        for _ in 0..12 {
            read(&mut t, 5, p);
        }
        assert_eq!(t.need, Some(need153 as f64), "{:#?}", t.logs);
        assert_eq!(t.level(), Some(153));
        assert!(!t.logs.iter().any(|l| l.contains("重新當基準")), "{:#?}", t.logs);
        let g0 = t.gain;
        read(&mut t, e + 100_000, p);
        read(&mut t, e + 200_000, p);
        assert_eq!(t.gain - g0, 200_000, "{:#?}", t.logs);
    }

    #[test]
    fn saved_garbage_need_is_dropped() {
        let need153 = exp_table::need(153).unwrap() as f64;
        let mut t = Tracker::new(default_cfg(), Some(1.8e9));
        t.load_state(&json!({"need": 19.334880123743233}));
        assert_eq!(t.need, None);
        t.load_state(&json!({"need": need153}));
        assert_eq!(t.need, Some(need153));
    }

    #[test]
    fn need_from_low_percent_snaps_to_the_table() {
        let mut t = Tracker::new(default_cfg(), Some(1.8e9));
        t.cfg.insert("exp_region".into(), json!([0, 0, 10, 10]));
        let need = exp_table::need(153).unwrap();
        let e = (need as f64 * 0.0137) as i64;
        read(&mut t, e, 1.37);
        read(&mut t, e, 1.37);
        assert_eq!(t.level(), Some(153));
        assert_eq!(t.need, Some(need as f64));
    }
}

#[cfg(test)]
mod qs_tests {
    use super::*;

    fn tracker() -> Tracker {
        let mut t = Tracker::new(default_cfg(), Some(1.8e9));
        t.cfg.insert("hp_region".into(), json!([0, 0, 10, 10]));
        t.running = true;
        t
    }

    fn feed(t: &mut Tracker, qs: Option<&str>, bag: Option<(&[i64], i64)>) {
        let out = OcrOut {
            qs: two(qs.map(|s| s.to_string()), None),
            inv: bag.map(|(cells, _)| two(Some(cells.iter().map(|&c| Some(c)).collect()), None)),
            bag_id: bag.map_or(0, |b| b.1),
            ..Default::default()
        };
        t.on_ocr(&out);
        t.fake_now = Some(t.now() + 3.0);
    }

    #[test]
    fn extra_digit_is_read_back() {
        let mut t = tracker();
        feed(&mut t, Some("158"), None);
        feed(&mut t, Some("157"), None);
        feed(&mut t, Some("157"), None);
        assert_eq!(t.qs_used[0], 1);
        feed(&mut t, Some("1571"), None);
        assert_eq!(t.qs[0], Some(157));
        assert_eq!(t.qs_used[0], 1);
        feed(&mut t, Some("1561"), None);
        feed(&mut t, Some("1561"), None);
        assert_eq!(t.qs[0], Some(156));
        assert_eq!(t.qs_used[0], 2);
        assert_eq!(t.logs.iter().filter(|l| l.contains("多讀了一位數")).count(), 1);
    }

    #[test]
    fn rise_without_drop_needs_more_reads() {
        let mut t = tracker();
        feed(&mut t, Some("500"), None);
        for _ in 0..7 {
            feed(&mut t, Some("900"), None);
        }
        assert_eq!(t.qs[0], Some(500));
        feed(&mut t, Some("900"), None);
        assert_eq!(t.qs[0], Some(900));
        assert_eq!(t.qs_used[0], 0);
    }

    #[test]
    fn rise_after_drop_is_quick() {
        let mut t = tracker();
        feed(&mut t, Some("500"), None);
        feed(&mut t, Some("499"), None);
        feed(&mut t, Some("499"), None);
        assert_eq!(t.qs_used[0], 1);
        feed(&mut t, Some("900"), None);
        feed(&mut t, Some("900"), None);
        assert_eq!(t.qs[0], Some(900));
        assert_eq!(t.qs_used[0], 1);
    }

    #[test]
    fn bag_confirms_quickslot() {
        let mut t = tracker();
        feed(&mut t, Some("300"), None);
        assert!(!t.qs_ok[0]);
        feed(&mut t, Some("300"), Some((&[3000, 300], 1)));
        assert!(t.qs_ok[0]);
        assert_eq!(t.qs[0], Some(300));
    }

    #[test]
    fn covered_bag_cell_keeps_the_total() {
        let mut t = tracker();
        let bag = |t: &mut Tracker, cells: &[i64], id: i64, n: usize| {
            for _ in 0..n {
                feed(t, Some("1344"), Some((cells, id)));
            }
        };
        bag(&mut t, &[3000, 3000, 1344], 1, 3);
        assert_eq!(t.est_total(0), Some(7344));
        bag(&mut t, &[3000, 1344], 2, 8);
        assert_eq!(t.est_total(0), Some(7344));
        assert_eq!(t.logs.iter().filter(|l| l.contains("先當作還在")).count(), 1);
        bag(&mut t, &[1344, 3000, 3000], 3, 4);
        assert_eq!(t.est_total(0), Some(7344));
        bag(&mut t, &[3000, 3000, 3000, 1344], 4, 6);
        assert_eq!(t.est_total(0), Some(10344));
        bag(&mut t, &[3000, 3000, 1344], 5, 6);
        assert_eq!(t.est_total(0), Some(10344));
        t.fake_now = Some(t.now() + 601.0);
        bag(&mut t, &[3000, 3000, 1344], 6, 3);
        assert_eq!(t.est_total(0), Some(7344));
        assert_eq!(t.used(0), 0);
    }

    #[test]
    fn missing_bag_cell_is_not_a_misread() {
        let mut t = tracker();
        for _ in 0..3 {
            feed(&mut t, None, Some((&[3000, 3000, 2871], 1)));
        }
        feed(&mut t, Some("2871"), None);
        for _ in 0..4 {
            feed(&mut t, Some("2871"), Some((&[3000, 2706], 2)));
        }
        assert_eq!(t.qs[0], Some(2871));
        assert_eq!(t.logs.iter().filter(|l| l.contains("對不上")).count(), 0);
    }

    #[test]
    fn wrong_quickslot_is_caught_by_the_bag() {
        let mut t = tracker();
        for _ in 0..3 {
            feed(&mut t, None, Some((&[157], 1)));
        }
        assert_eq!(t.total[0].map(|x| x.0), Some(157));
        feed(&mut t, Some("1572"), None);
        feed(&mut t, Some("1562"), None);
        feed(&mut t, Some("1562"), None);
        assert_eq!(t.qs_used[0], 10);
        for _ in 0..6 {
            feed(&mut t, Some("1562"), Some((&[156], 2)));
        }
        assert_eq!(t.logs.iter().filter(|l| l.contains("對不上")).count(), 1);
        assert_eq!(t.total[0].map(|x| x.0), Some(156));
        assert_eq!(t.qs_used[0], 1);
    }

    #[test]
    fn short_read_picks_the_matching_cell() {
        let mut t = tracker();
        for _ in 0..3 {
            feed(&mut t, None, Some((&[2828, 3000, 1765], 1)));
        }
        for id in 2..6 {
            feed(&mut t, Some("28"), Some((&[2828, 3000, 1765], id)));
        }
        assert_eq!(t.qs[0], Some(2828));
        assert!(t.alert.is_none());
    }

    #[test]
    fn one_digit_cell_is_not_a_match() {
        let mut t = tracker();
        for _ in 0..3 {
            feed(&mut t, None, Some((&[3000, 3000, 1, 2706, 3000], 1)));
        }
        for id in 2..6 {
            feed(&mut t, Some("1611"), Some((&[3000, 3000, 1, 2706, 3000], id)));
        }
        assert_ne!(t.qs[0], Some(1));
    }

    #[test]
    fn wrong_frame_asks_to_reframe() {
        let mut t = tracker();
        for _ in 0..3 {
            feed(&mut t, None, Some((&[2828, 3000, 1765], 1)));
        }
        for id in 2..16 {
            feed(&mut t, Some("2986"), Some((&[2828, 3000, 1765], id)));
        }
        assert!(t.alert.as_ref().map_or(false, |a| a.0.contains("框錯位置")));
        assert_eq!(t.logs.iter().filter(|l| l.contains("框錯位置")).count(), 1);
    }

    #[test]
    fn reframe_resets_wrong_frame_count() {
        let mut t = tracker();
        for _ in 0..3 {
            feed(&mut t, None, Some((&[2828, 3000, 1765], 1)));
        }
        for id in 2..4 {
            feed(&mut t, Some("2986"), Some((&[2828, 3000, 1765], id)));
        }
        t.forget_qs(0);
        for id in 4..7 {
            feed(&mut t, Some("2986"), Some((&[2828, 3000, 1765], id)));
        }
        assert!(t.logs.iter().any(|l| l.contains("改用背包裡的")));
        assert!(!t.logs.iter().any(|l| l.contains("框錯位置")));
    }

    #[test]
    fn both_quickslots_lost_asks_to_check() {
        let mut t = tracker();
        let read = |t: &mut Tracker, a: &str, b: &str| {
            let out = OcrOut { exp: Some("12345678[50.00%]".into()), qs: two(Some(a.into()), Some(b.into())), ..Default::default() };
            t.on_ocr(&out);
            t.fake_now = Some(t.now() + 3.0);
        };
        for _ in 0..3 {
            read(&mut t, "", "");
        }
        read(&mut t, "513", "");
        read(&mut t, "513", "");
        for _ in 0..12 {
            read(&mut t, "", "");
        }
        assert_eq!(t.logs.iter().filter(|l| l.contains("快捷欄讀取失敗")).count(), 1);
        for _ in 0..12 {
            read(&mut t, "", "");
        }
        assert_eq!(t.logs.iter().filter(|l| l.contains("快捷欄讀取失敗")).count(), 1);
    }

    #[test]
    fn closed_quickslot_noise_still_counts_as_lost() {
        let mut t = tracker();
        let read = |t: &mut Tracker, a: &str, b: &str| {
            let out = OcrOut { exp: Some("12345678[50.00%]".into()), qs: two(Some(a.into()), Some(b.into())), ..Default::default() };
            t.on_ocr(&out);
            t.fake_now = Some(t.now() + 3.0);
        };
        for i in 0..12 {
            if i % 2 == 0 {
                read(&mut t, "1", "");
            } else {
                read(&mut t, "", "1010");
            }
        }
        assert_eq!(t.logs.iter().filter(|l| l.contains("快捷欄讀取失敗")).count(), 1);
        let mut t = tracker();
        for i in 0..12 {
            read(&mut t, if i < 8 { "1" } else { "0" }, if i % 3 == 0 { "" } else { "1" });
        }
        assert_eq!(t.logs.iter().filter(|l| l.contains("快捷欄讀取失敗")).count(), 1);
    }

    #[test]
    fn closed_quickslot_noise_is_not_used() {
        let mut t = tracker();
        let read = |t: &mut Tracker, a: &str, b: &str| {
            let out = OcrOut { exp: Some("12345678[50.00%]".into()), qs: two(Some(a.into()), Some(b.into())), ..Default::default() };
            t.on_ocr(&out);
            t.fake_now = Some(t.now() + 3.0);
        };
        for v in ["1", "", "1", "0", "0", "1", "5", "5", "5", "1", "5", "5"] {
            read(&mut t, v, "");
        }
        for _ in 0..9 {
            read(&mut t, "2632", "2995");
        }
        for _ in 0..3 {
            read(&mut t, "2630", "2995");
        }
        assert_eq!(t.qs_used[0], 2);
    }

    #[test]
    fn steady_reading_is_not_lost() {
        let mut t = tracker();
        let read = |t: &mut Tracker, a: &str, b: &str| {
            let out = OcrOut { exp: Some("12345678[50.00%]".into()), qs: two(Some(a.into()), Some(b.into())), ..Default::default() };
            t.on_ocr(&out);
            t.fake_now = Some(t.now() + 3.0);
        };
        for i in 0..20 {
            read(&mut t, &(2000 - i).to_string(), "");
        }
        assert!(!t.logs.iter().any(|l| l.contains("快捷欄讀取失敗")));
    }

    #[test]
    fn one_mismatch_is_not_wrong_frame() {
        let mut t = tracker();
        for _ in 0..3 {
            feed(&mut t, None, Some((&[2828, 3000, 1765], 1)));
        }
        for id in 2..4 {
            feed(&mut t, Some("2986"), Some((&[2828, 3000, 1765], id)));
        }
        assert_eq!(t.qs[0], Some(2828));
        assert!(!t.logs.iter().any(|l| l.contains("框錯位置")));
    }

    #[test]
    fn one_digit_misread_is_not_wrong_frame() {
        let mut t = tracker();
        for _ in 0..3 {
            feed(&mut t, None, Some((&[2828, 3000, 1765], 1)));
        }
        for id in 2..6 {
            feed(&mut t, Some("2826"), Some((&[2828, 3000, 1765], id)));
        }
        assert!(!t.logs.iter().any(|l| l.contains("框錯位置")));
    }

    #[test]
    fn covered_cell_in_use_is_not_usage() {
        let mut t = tracker();
        let mut q = 64;
        for _ in 0..3 {
            feed(&mut t, Some("64"), Some((&[3000, 64], 1)));
        }
        for i in 0..40 {
            if i % 2 == 1 {
                q -= 1;
            }
            feed(&mut t, Some(&q.to_string()), Some((&[3000], 2)));
        }
        for _ in 0..6 {
            feed(&mut t, Some(&q.to_string()), Some((&[3000, q], 3)));
        }
        assert_eq!(t.used(0), 64 - q);
        assert!(!t.logs.iter().any(|l| l.contains("補算")));
    }

    #[test]
    fn extra_digit_on_almost_empty_stack_is_not_a_switch() {
        let mut t = tracker();
        for _ in 0..3 {
            feed(&mut t, Some("16"), Some((&[3000, 16], 1)));
        }
        for v in [15, 15, 14, 14] {
            feed(&mut t, Some(&v.to_string()), Some((&[3000, v], 2)));
        }
        for v in ["113", "113", "113", "13", "13", "12", "12"] {
            feed(&mut t, Some(v), None);
        }
        assert_eq!(t.qs_used[0], 4);
        assert!(!t.logs.iter().any(|l| l.contains("換下一格")));
        for v in ["1", "1", "3000", "3000"] {
            feed(&mut t, Some(v), None);
        }
        assert_eq!(t.qs_used[0], 16);
        assert_eq!(t.logs.iter().filter(|l| l.contains("換下一格")).count(), 1);
    }

    #[test]
    fn both_potions_losing_cells_at_once_is_a_cover() {
        let mut t = Tracker::new(default_cfg(), Some(1.8e9));
        t.cfg.insert("inv_region".into(), json!([0, 0, 100, 100]));
        t.running = true;
        let bag = |t: &mut Tracker, hp: &[i64], mp: &[i64], id: i64, n: usize| {
            for _ in 0..n {
                let out = OcrOut {
                    inv: Some(two(Some(hp.iter().map(|&c| Some(c)).collect()), Some(mp.iter().map(|&c| Some(c)).collect()))),
                    bag_id: id,
                    ..Default::default()
                };
                t.on_ocr(&out);
                t.fake_now = Some(t.now() + 3.0);
            }
        };
        bag(&mut t, &[3000, 40], &[3000, 3000, 25], 1, 4);
        bag(&mut t, &[3000, 38], &[3000, 3000, 24], 1, 3);
        assert_eq!((t.used(0), t.used(1)), (2, 1));
        bag(&mut t, &[3000], &[3000, 3000], 1, 40);
        assert_eq!((t.used(0), t.used(1)), (2, 1));
        assert_eq!(t.est_total(0), Some(3038));
        bag(&mut t, &[3000, 30], &[3000, 3000, 20], 1, 4);
        assert_eq!((t.used(0), t.used(1)), (10, 5));
    }

    #[test]
    fn periodic_cover_keeps_usage() {
        let mut t = tracker();
        t.cfg.insert("mp_region".into(), json!([0, 20, 10, 10]));
        let (mut hp, mut mp) = (3260i64, 13309i64);
        let start = (hp, mp);
        for i in 0..1000 {
            if i >= 10 && i % 2 == 1 {
                hp -= 1;
            }
            if i >= 10 && i % 33 == 32 {
                mp -= 1;
            }
            let hp_use = if hp > 3000 { hp - 3000 } else { hp };
            let mp_use = mp - 1584 - 9000;
            let covered = i % 86 >= 70;
            let hp_cells: Vec<i64> = if hp <= 3000 { vec![hp] } else if covered { vec![3000] } else { vec![3000, hp_use] };
            let mp_cells: Vec<i64> = if covered { vec![1584, mp_use, 3000] } else { vec![1584, mp_use, 3000, 3000, 3000] };
            let out = OcrOut {
                qs: two(Some(hp_use.to_string()), Some(mp_use.to_string())),
                inv: Some(two(Some(hp_cells.into_iter().map(Some).collect()), Some(mp_cells.into_iter().map(Some).collect()))),
                bag_id: 1,
                ..Default::default()
            };
            t.on_ocr(&out);
            t.fake_now = Some(t.now() + 3.0);
        }
        let (real_hp, real_mp) = (start.0 - hp, start.1 - mp);
        assert!((t.used(0) - real_hp).abs() <= 3, "馴鹿奶 算到 {} 實際 {real_hp}", t.used(0));
        assert!((t.used(1) - real_mp).abs() <= 3, "紅豆刨冰 算到 {} 實際 {real_mp}", t.used(1));
        assert!(!t.logs.iter().any(|l| l.contains("改用背包裡的") || l.contains("對帳")), "{:#?}", t.logs);
    }
}

#[cfg(test)]
mod anchor_tests {
    use super::*;

    fn tracker() -> Tracker {
        let mut t = Tracker::new(default_cfg(), Some(1.8e9));
        t.cfg.insert("hp_region".into(), json!([0, 0, 10, 10]));
        t.cfg.insert("inv_region".into(), json!([0, 0, 100, 100]));
        t.tpl_exists = two(true, true);
        t.running = true;
        t
    }

    fn bag_only() -> Tracker {
        let mut t = tracker();
        t.cfg.remove("hp_region");
        t
    }

    fn feed(t: &mut Tracker, qs: Option<&str>, bag: Option<&[i64]>) {
        let out = OcrOut {
            exp: Some("12345678[50.00%]".into()),
            qs: two(qs.map(|s| s.to_string()), None),
            inv: bag.map(|cells| two(Some(cells.iter().map(|&c| Some(c)).collect()), None)),
            bag_id: 1,
            ..Default::default()
        };
        t.on_ocr(&out);
        t.fake_now = Some(t.now() + 3.0);
    }

    fn feed_n(t: &mut Tracker, n: usize, qs: Option<&str>, bag: Option<&[i64]>) {
        for _ in 0..n {
            feed(t, qs, bag);
        }
    }

    #[test]
    fn quickslot_misread_is_undone_at_the_next_anchor() {
        let mut t = tracker();
        feed_n(&mut t, 3, Some("300"), Some(&[3000, 300]));
        feed_n(&mut t, 2, Some("290"), None);
        assert_eq!(t.used(0), 10);
        feed_n(&mut t, 4, Some("290"), Some(&[3000, 300]));
        assert_eq!(t.used(0), 0, "{:#?}", t.logs);
        feed_n(&mut t, 3, Some("300"), Some(&[3000, 300]));
        assert_eq!(t.used(0), 0);
        assert_eq!(t.est_total(0), Some(3300));
    }

    #[test]
    fn blind_quickslot_is_filled_from_the_bag_once() {
        let mut t = tracker();
        feed_n(&mut t, 3, Some("500"), Some(&[3000, 500]));
        feed_n(&mut t, 8, Some(""), None);
        feed_n(&mut t, 3, Some(""), Some(&[3000, 488]));
        assert_eq!(t.used(0), 12, "{:#?}", t.logs);
        feed_n(&mut t, 3, Some("488"), None);
        feed_n(&mut t, 2, Some("487"), None);
        assert_eq!(t.used(0), 13);
    }

    #[test]
    fn bag_mode_small_cell_covered_is_refunded_when_it_returns() {
        let mut t = bag_only();
        feed_n(&mut t, 3, None, Some(&[3000, 40]));
        feed_n(&mut t, 2, None, Some(&[3000, 38]));
        assert_eq!(t.used(0), 2);
        t.fake_now = Some(t.now() + 120.0);
        feed_n(&mut t, 3, None, Some(&[3000]));
        assert_eq!(t.used(0), 40);
        t.fake_now = Some(t.now() + 60.0);
        feed_n(&mut t, 4, None, Some(&[3000, 35]));
        assert_eq!(t.used(0), 5, "{:#?}", t.logs);
    }

    #[test]
    fn covered_full_cell_still_counts_visible_drinking() {
        let mut t = bag_only();
        feed_n(&mut t, 3, None, Some(&[3000, 3000, 900]));
        let mut v = 900;
        for i in 0..40 {
            if i % 2 == 1 {
                v -= 1;
            }
            feed(&mut t, None, Some(&[3000, v]));
        }
        feed(&mut t, None, Some(&[3000, v]));
        assert_eq!(t.est_total(0), Some(6000 + v));
        assert_eq!(t.used(0), 900 - v, "{:#?}", t.logs);
        feed_n(&mut t, 3, None, Some(&[3000, 3000, v]));
        assert_eq!(t.used(0), 900 - v);
        assert!(!t.logs.iter().any(|l| l.contains("又出現了")), "{:#?}", t.logs);
    }

    #[test]
    fn bag_mode_used_up_cell_is_not_a_cover() {
        let mut t = bag_only();
        feed_n(&mut t, 3, None, Some(&[3000, 3000, 6]));
        feed_n(&mut t, 2, None, Some(&[3000, 3000, 3]));
        t.fake_now = Some(t.now() + 30.0);
        feed_n(&mut t, 2, None, Some(&[3000, 2998]));
        assert_eq!(t.used(0), 8, "{:#?}", t.logs);
        assert_eq!(t.est_total(0), Some(5998));
        assert!(!t.logs.iter().any(|l| l.contains("被擋住")));
    }

    #[test]
    fn odd_increase_is_a_purchase_not_usage() {
        let mut t = tracker();
        feed_n(&mut t, 3, Some("300"), Some(&[3000, 300]));
        feed_n(&mut t, 2, Some("298"), Some(&[3000, 298]));
        feed_n(&mut t, 4, Some("298"), Some(&[3000, 298, 7]));
        assert_eq!(t.used(0), 2);
        assert_eq!(t.est_total(0), Some(3305));
        assert!(t.logs.iter().any(|l| l.contains("當作新的總數")));
    }

    #[test]
    fn flat_quickslot_with_falling_bag_asks_to_check_the_frame() {
        let mut t = tracker();
        feed_n(&mut t, 3, Some("3000"), Some(&[3000, 500]));
        let mut v = 500;
        for i in 0..60 {
            if i % 3 == 2 {
                v -= 1;
            }
            feed(&mut t, Some("3000"), Some(&[3000, v]));
        }
        assert!((500 - v) - t.used(0) <= 5, "算到 {} 實際 {}", t.used(0), 500 - v);
        assert!(t.alert.as_ref().map_or(false, |a| a.0.contains("一段時間未減少")), "{:#?}", t.logs);
    }

    #[test]
    fn small_drift_is_added_when_pausing() {
        let mut t = tracker();
        t.running = false;
        t.toggle_run(0.0);
        feed_n(&mut t, 3, Some("3000"), Some(&[3000, 500]));
        feed_n(&mut t, 2, Some("3000"), Some(&[3000, 497]));
        assert_eq!(t.used(0), 0);
        t.toggle_run(0.0);
        assert_eq!(t.used(0), 3, "{:#?}", t.logs);
    }

    #[test]
    fn sudden_implausible_bag_drop_waits() {
        let mut t = tracker();
        feed_n(&mut t, 3, Some("117"), Some(&[3000, 117]));
        feed_n(&mut t, 2, Some("117"), Some(&[3000, 57]));
        assert_eq!(t.est_total(0), Some(3117));
        feed_n(&mut t, 2, Some("117"), Some(&[3000, 117]));
        assert_eq!(t.est_total(0), Some(3117));
        assert!(!t.logs.iter().any(|l| l.contains("不像是喝掉的")), "{:#?}", t.logs);
    }

    #[test]
    fn sold_stack_is_not_usage() {
        let mut t = tracker();
        feed_n(&mut t, 3, Some("500"), Some(&[3000, 500]));
        feed_n(&mut t, 3, Some("500"), Some(&[500]));
        assert_eq!(t.est_total(0), Some(3500));
        t.fake_now = Some(t.now() + 601.0);
        feed_n(&mut t, 3, Some("500"), Some(&[500]));
        assert_eq!(t.est_total(0), Some(500));
        assert_eq!(t.used(0), 0);
    }

    #[test]
    fn rate_cap_follows_the_player() {
        let mut t = tracker();
        assert_eq!(t.rate_cap(0), 90.0);
        for i in 0..240 {
            t.series.push((i as f64 * 5.0, 0, i as i64 * 100 / 120, 0, None, [0; NP]));
        }
        assert_eq!(t.rate_cap(0), 20.0);
        t.series.clear();
        for i in 0..240 {
            t.series.push((i as f64 * 5.0, 0, i as i64 * 2, 0, None, [0; NP]));
        }
        assert_eq!(t.rate_cap(0), 48.0);
    }

    #[test]
    fn anchor_state_survives_save_and_load() {
        let mut t = tracker();
        feed_n(&mut t, 3, Some("300"), Some(&[3000, 300]));
        feed_n(&mut t, 2, Some("290"), None);
        let data = t.state_json();
        let mut u = tracker();
        u.load_state(&data);
        assert_eq!(u.anchor_used, t.anchor_used);
        assert_eq!(u.total, t.total);
        assert_eq!(u.used(0), 10);
    }

    #[test]
    fn csv_rows_can_be_removed_and_restored() {
        let dir = std::env::temp_dir().join(format!("csv_edit_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("練功紀錄.csv");
        let _ = std::fs::remove_file(&p);
        let t = tracker();
        let head = t.csv_header();
        let row = |start: &str| {
            let mut r = vec![start.to_string()];
            r.extend(["30.0", "6000000", "12000000", "1", "0", "60", "5", "400000", "800000", "150", "500000", "100000", "200000"].map(String::from));
            r
        };
        write_csv_row(&p, &head, &row("2026-10-06 08:00")).unwrap();
        write_csv_row(&p, &head, &row("2026-10-06 09:00")).unwrap();
        let before = std::fs::read(&p).unwrap();
        let gone = remove_csv_lines(&p, &|l| l.starts_with("2026-10-06 08:00")).unwrap();
        assert_eq!(gone.len(), 1);
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.starts_with('\u{feff}'));
        assert!(!text.contains("08:00") && text.contains("09:00"));
        assert!(remove_csv_lines(&p, &|l| l.starts_with("2026-01-01")).unwrap().is_empty());
        restore_csv_lines(&p, &head, &gone).unwrap();
        let mut a_lines: Vec<String> = String::from_utf8_lossy(&before).lines().map(String::from).collect();
        let mut b_lines: Vec<String> = std::fs::read_to_string(&p).unwrap().lines().map(String::from).collect();
        a_lines.sort();
        b_lines.sort();
        assert_eq!(a_lines, b_lines);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn curve_is_saved_with_the_session_and_written_on_log() {
        let dir = std::env::temp_dir().join(format!("curve_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut t = tracker();
        t.log_path = Some(dir.join("練功紀錄.csv"));
        t.running = false;
        t.toggle_run(0.0);
        for i in 0..20 {
            t.fake_now = Some(t.now() + 10.0);
            t.gain += 1000;
            if i == 5 {
                t.qs_used[0] += 3;
            }
            t.tick();
        }
        assert!(t.curve.len() >= 6, "{:?}", t.curve);
        let mut u = tracker();
        u.load_state(&t.state_json());
        assert_eq!(u.curve.len(), t.curve.len());
        assert_eq!(t.log_session(), Saved::Main);
        let text = std::fs::read_to_string(dir.join(CURVE_FILE)).unwrap();
        let v: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        let pts = parse_curve(&v["pts"]);
        assert_eq!(pts.last().unwrap().1, 20_000);
        assert_eq!(pts.last().unwrap().2, 3);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
