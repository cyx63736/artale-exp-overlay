pub mod util;

use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use util::*;

pub const KS: [&str; 2] = ["hp", "mp"];
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

#[derive(Clone, Debug, PartialEq)]
pub struct Drop {
    pub miss: i64,
    pub t: f64,
    pub n: i64,
    pub credit: Option<i64>,
    pub slots: Option<Option<Slots>>,
}

impl Drop {
    fn len(&self) -> usize {
        if self.slots.is_some() {
            5
        } else if self.credit.is_some() {
            4
        } else {
            3
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct OcrOut {
    pub exp: Option<String>,
    pub exp_err: Option<String>,
    pub qs: [Option<String>; 2],
    pub qs_err: [Option<String>; 2],
    pub inv: Option<[Option<Vec<Option<i64>>>; 2]>,
    pub meso: Option<String>,
    pub meso_box_only: bool,
    pub bag_id: i64,
    pub qs_sig: [Option<Vec<f32>>; 2],
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

pub type Snap = (f64, i64, i64, i64, Option<i64>);

pub struct Tracker {
    pub cfg: Map<String, Value>,
    pub fake_now: Option<f64>,
    pub logs: Vec<String>,
    pub ui: Vec<Ui>,
    pub tpl_exists: [bool; 2],
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
    pub qs: [Option<i64>; 2],
    pub qs_pend: [Option<(i64, u32)>; 2],
    pub qs_t: [f64; 2],
    pub qs_seen: [f64; 2],
    pub qs_credit: [i64; 2],
    pub qs_rise: [Option<(i64, f64)>; 2],
    pub fill_from: f64,
    pub qs_all: [i64; 2],
    pub qs_add: [i64; 2],
    pub total: [Option<(i64, i64, i64, f64)>; 2],
    pub inv_pend: [Option<(i64, i64, u32)>; 2],
    pub inv_slots: [Option<i64>; 2],
    pub total_act: [f64; 2],
    pub last_rise: [Option<(i64, i64, f64, i64, i64)>; 2],
    pub cap_since: [Option<f64>; 2],
    pub meso: Option<i64>,
    pub meso_pend: Option<(i64, u32, BTreeSet<i64>)>,
    pub meso_drop: Option<(i64, i64, f64)>,
    pub meso_rej: Option<i64>,
    pub meso_t: f64,
    pub qs_sus: [i64; 2],
    pub exp_ok_t: f64,
    pub exp_move_t: f64,
    pub auto_paused: bool,
    pub acc: f64,
    pub run_start: f64,
    pub started_at: Option<String>,
    pub gain: i64,
    pub hist: Vec<(f64, i64)>,
    pub levelups: i64,
    pub deaths: i64,
    pub qs_used: [i64; 2],
    pub bag_used: [i64; 2],
    pub meso_anchor: Option<(i64, f64, Option<f64>)>,
    pub inc_done: Option<(i64, f64)>,
    pub seg_pending: bool,
    pub pend_dec: Option<(i64, Option<f64>)>,
    pub reject_n: u32,
    pub pre_inc: Option<(i64, usize, f64)>,
    pub rej_run: Option<(i64, i64, u32)>,
    pub series: Vec<Snap>,
    pub qs_sus_cnt: [i64; 2],
    pub last_drop: [Option<Drop>; 2],
    pub recon_acc: [i64; 2],
    pub recon_add: [i64; 2],
    pub recon_t: [Option<f64>; 2],
    pub seg_t: f64,
    pub cost_prev: [f64; 2],
    pub used_mark: [i64; 2],
    pub prev_price: [Option<f64>; 2],
    pub session_no: u32,
}

pub fn sys_now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

const SESSION_KEYS: [&str; 17] = [
    "gain", "hist", "levelups", "deaths", "qs_used", "bag_used", "meso_anchor", "series", "qs_sus_cnt", "last_drop", "inc_done",
    "seg_pending", "recon_acc", "recon_add", "cost_prev", "used_mark", "prev_price",
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
            tpl_exists: [false; 2],
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
            qs: [None; 2],
            qs_pend: [None; 2],
            qs_t: [0.0; 2],
            qs_seen: [0.0; 2],
            qs_credit: [0; 2],
            qs_rise: [None; 2],
            fill_from: now,
            qs_all: [0; 2],
            qs_add: [0; 2],
            total: [None; 2],
            inv_pend: [None; 2],
            inv_slots: [None; 2],
            total_act: [0.0; 2],
            last_rise: [None; 2],
            cap_since: [None; 2],
            meso: None,
            meso_pend: None,
            meso_drop: None,
            meso_rej: None,
            meso_t: 0.0,
            qs_sus: [0; 2],
            exp_ok_t: 0.0,
            exp_move_t: now,
            auto_paused: false,
            acc: 0.0,
            run_start: 0.0,
            started_at: None,
            gain: 0,
            hist: vec![],
            levelups: 0,
            deaths: 0,
            qs_used: [0; 2],
            bag_used: [0; 2],
            meso_anchor: None,
            inc_done: None,
            seg_pending: false,
            pend_dec: None,
            reject_n: 0,
            pre_inc: None,
            rej_run: None,
            series: vec![],
            qs_sus_cnt: [0; 2],
            last_drop: [None, None],
            recon_acc: [0; 2],
            recon_add: [0; 2],
            recon_t: [None; 2],
            seg_t: now,
            cost_prev: [0.0; 2],
            used_mark: [0; 2],
            prev_price: [None; 2],
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
        self.qs_used = [0; 2];
        self.bag_used = [0; 2];
        self.meso_anchor = None;
        self.inc_done = None;
        self.seg_pending = false;
        self.meso_drop = None;
        self.pend_dec = None;
        self.reject_n = 0;
        self.pre_inc = None;
        self.rej_run = None;
        self.cap_since = [None; 2];
        self.series = vec![];
        self.qs_sus_cnt = [0; 2];
        self.last_drop = [None, None];
        self.recon_acc = [0; 2];
        self.recon_add = [0; 2];
        self.recon_t = [None; 2];
        self.seg_t = self.now();
        self.cost_prev = [0.0; 2];
        self.used_mark = [0; 2];
        self.prev_price = [None; 2];
        self.session_no += 1;
    }

    pub fn snapshot(&self, a: f64) -> Snap {
        (a, self.gain, self.used(0), self.used(1), self.income())
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
        let ks: &[usize] = match k {
            Some(0) => &[0],
            Some(_) => &[1],
            None => &[0, 1],
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
                }
            }
        }
    }

    pub fn close_seg(&mut self, act_end: f64) {
        let (Some(ma), Some(m)) = (self.meso_anchor, self.meso) else { return };
        let d = self.inc_done.unwrap_or((0, 0.0));
        self.inc_done = Some((d.0 + m - ma.0, d.1 + (act_end - ma.1).max(0.0)));
        self.meso_anchor = None;
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
        if self.counting() && self.meso_anchor.is_none() {
            self.meso_anchor = Some((v, self.active(), Some(self.meso_t)));
            self.log(format!("楓幣 {} 當作這次的起點", comma(v)));
        }
        "ok"
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
            self.need = need_est;
        } else if let Some(ne) = need_est.filter(|&n| n != 0.0) {
            if pct.unwrap_or(0.0) >= 3.0 || self.need.map_or(true, |n| n == 0.0) {
                self.need = Some(ne);
            }
        }
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
            }
            self.reject_n = 0;
            self.pend_dec = None;
            self.rej_run = None;
            self.pre_inc = None;
        }
        false
    }

    fn qs_use(&mut self, k: usize, n: i64, sus: bool) {
        let c = n.min(self.qs_credit[k]);
        self.qs_credit[k] = 0;
        let n = n - c;
        if n <= 0 {
            return;
        }
        self.qs_all[k] += n;
        if sus || n >= 20 {
            self.qs_sus[k] += n;
        }
        if self.counting() {
            self.qs_used[k] += n;
            if sus || n >= 20 {
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
        let gap = now - self.qs_seen[k];
        self.qs_seen[k] = now;
        if last.is_none() || last == Some(val) {
            self.qs[k] = Some(val);
            self.qs_pend[k] = None;
            self.qs_t[k] = now;
            self.qs_credit[k] = 0;
            return;
        }
        let last = last.unwrap();
        let step = (15f64).max((now - self.qs_t[k]) / 60.0 * 90.0);
        let mut p = self.qs_pend[k];
        if p.is_some() && gap > self.blind_sec() {
            p = None;
            self.qs_pend[k] = None;
        }
        if let Some(pp) = p {
            if val < pp.0 && pp.0 < last && (last - val) as f64 <= step {
                let n = self.qs_drop(k, last, pp.0, now);
                self.qs_use(k, n, false);
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
            if last as f64 <= step {
                let extra = if cap != 0 && (cap as f64 - step) <= val as f64 && val <= cap { cap - val } else { 0 };
                let n = (last + extra).max(0);
                self.qs_use(k, n, true);
                self.qs_rise[k] = None;
                self.log(format!("{name} 快捷欄 {last} → {val}（這格用完換下一格，算用掉 {n}）"));
            } else {
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
        } else if cnt < 8 {
            return;
        } else {
            self.qs_rise[k] = None;
            self.log(format!("{name} 快捷欄 {last} → {val}（一下子少太多，當成換格顯示，不算用量）"));
        }
        self.qs[k] = Some(val);
        self.qs_pend[k] = None;
        self.qs_t[k] = now;
        self.qs_credit[k] = 0;
    }

    fn qs_miss(&mut self, k: usize, miss: i64, now: f64, slots: Option<Slots>) -> Option<i64> {
        let name = self.name(k);
        let prev = self.total[k].unwrap();
        let blind_for = now - self.qs_t[k];
        if self.qs[k].is_some() && blind_for <= self.blind_sec() {
            let lim = (15f64).max((self.active() - self.total_act[k]) / 60.0 * 90.0);
            return if miss as f64 <= lim { Some(-1) } else { None };
        }
        if prev.3 < self.fill_from || miss as f64 > (15f64).max((now - prev.3).min(blind_for) / 60.0 * 90.0) {
            self.last_drop[k] = Some(Drop { miss, t: now, n: 0, credit: Some(0), slots: Some(slots) });
            self.log(format!("{name} 快捷欄讀不到時背包少了 {}，不像是喝掉的，不算用量", comma(miss)));
            return None;
        }
        self.qs_credit[k] += miss;
        self.qs_pend[k] = None;
        let x = (self.active() - self.total_act[k]).max(0.0);
        let n = 0.max(miss.min(pround(miss as f64 * x / (now - prev.3).max(1.0))));
        self.bag_used[k] += n;
        self.last_drop[k] = Some(Drop { miss, t: now, n, credit: Some(miss), slots: Some(slots) });
        if n != 0 {
            self.log(format!("{name} 快捷欄讀不到的這段時間用掉 {} 瓶（用背包的數字補算）", comma(n)));
        }
        Some(n)
    }

    fn reconcile(&mut self, k: usize, miss: i64, now: f64, slots: Option<Slots>) {
        let name = self.name(k);
        let mut acc = self.recon_acc[k];
        if miss > 0 {
            if self.total[k].map_or(false, |t| t.3 >= self.seg_t) {
                acc += miss;
            }
        } else if miss < 0 {
            acc = (acc + miss).max(-self.recon_add[k] - 5);
        }
        self.recon_acc[k] = acc;
        if acc > 5 {
            if let Some(s) = slots {
                if s.0.map_or(false, |a| s.1 < a) {
                    return;
                }
            }
            match self.recon_t[k] {
                None => self.recon_t[k] = Some(now),
                Some(t) if now - t >= self.blind_sec() && self.qs_pend[k].is_none() => {
                    self.bag_used[k] += acc;
                    self.recon_add[k] += acc;
                    self.recon_acc[k] = 0;
                    self.recon_t[k] = None;
                    self.log(format!("{name} 對帳：背包實際少掉的比算到的多 {} 瓶（快捷欄漏算），補算", comma(acc)));
                }
                _ => {}
            }
            return;
        }
        self.recon_t[k] = None;
        if acc < -5 && self.recon_add[k] > 0 {
            let back = self.recon_add[k].min(-acc);
            self.bag_used[k] -= back;
            self.recon_add[k] -= back;
            self.recon_acc[k] += back;
            self.log(format!("{name} 對帳：算到的用量比背包少掉的多 {} 瓶，退回之前補算的 {}", comma(-acc), comma(back)));
        }
    }

    pub fn blind_sec(&self) -> f64 {
        let o = self.cfg_f("ocr_sec");
        (8f64).max(2.0 * if o == 0.0 { 3.0 } else { o } + 2.0)
    }

    fn drop_back(&self, ld: &Drop, slots: Option<Slots>, amt: i64, bag: bool, now: f64) -> bool {
        let s = ld.slots.flatten();
        let cells_back = match (s, slots) {
            (Some(s), Some(sl)) => s.0.map_or(false, |a| a != 0 && s.1 < a) && sl.1 > s.1,
            _ => false,
        };
        if (amt - ld.miss).abs() <= 5 {
            let credit = ld.credit.unwrap_or(0);
            return bag || ld.len() < 5 || cells_back || (credit == 0 && ld.miss <= 15);
        }
        amt < ld.miss && cells_back && (ld.miss - amt) as f64 <= (15f64).max((now - ld.t) / 60.0 * 90.0)
    }

    pub fn set_total(&mut self, k: usize, tv: i64, slots: Option<Slots>) {
        let (est, now) = (self.est_total(k), self.now());
        let name = self.name(k);
        if let Some(mut est) = est {
            let (lr, ld) = (self.last_rise[k], self.last_drop[k].clone());
            let amt = tv - est;
            let bag = !self.is_qs_mode(k);
            if amt > 5 && self.qs_sus[k] >= amt - 5 {
                let back = amt.min(self.qs_sus_cnt[k]);
                self.qs_used[k] -= back;
                let tail = if back != 0 { format!("，退回用量 {}", comma(back)) } else { String::new() };
                self.log(format!("{name} 背包總數 {}，預估 {}（多 {}）→ 快捷欄之前讀錯多扣了{tail}", comma(tv), comma(est), comma(amt)));
            } else if amt > 5 && ld.as_ref().map_or(false, |ld| now - ld.t < 1800.0 && self.drop_back(ld, slots, amt, bag, now)) {
                let ld = ld.unwrap();
                let mut back = amt.min(ld.n);
                self.bag_used[k] -= back;
                if ld.len() > 3 {
                    self.qs_credit[k] = (self.qs_credit[k] - amt.min(ld.credit.unwrap_or(0))).max(0);
                }
                if bag && ld.len() > 4 && ld.miss - amt > ld.n - back {
                    let extra = ld.miss - amt - (ld.n - back);
                    self.bag_used[k] += extra;
                    back -= extra;
                }
                self.last_drop[k] = None;
                let tail = if back > 0 {
                    format!("→ 退回用量 {}", comma(back))
                } else if back < 0 {
                    format!("→ 補算用量 {}", comma(-back))
                } else {
                    "→ 不算補貨".into()
                };
                self.log(format!("{name} 背包總數 {}，上次少掉的 {} 又出現了 {}（當時被擋住）{tail}", comma(tv), comma(ld.miss), comma(amt)));
            } else if amt > 5 {
                self.last_rise[k] = Some((amt, est, now, self.qs_all[k], self.qs_add[k]));
                self.log(format!("{name} 背包總數 {}，預估 {}（多 {}）→ 不算用量，當作新的總數", comma(tv), comma(est), comma(amt)));
            } else {
                if let Some((amt0, est0, t0, q0, a0)) = lr {
                    if now - t0 < 1800.0 {
                        let est0_now = est0 - (self.qs_all[k] - q0) + (self.qs_add[k] - a0);
                        let qs_live = !bag && self.qs[k].is_some() && now - self.qs_t[k] <= self.blind_sec();
                        let tol = 5 + if bag { 30 } else { 0 };
                        if (tv - est0_now).abs() <= tol && (qs_live || (est - tv) as f64 > (15f64).max((now - t0) / 60.0 * 90.0)) {
                            self.last_rise[k] = None;
                            self.log(format!("{name} 背包總數 {}，回到變多之前的數字 → 上次多出來的 {} 是讀錯，不算用量", comma(tv), comma(amt0)));
                            est = est0_now;
                        } else if (tv - est).abs() <= 5 {
                            self.last_rise[k] = None;
                        }
                    }
                }
                if tv != est {
                    self.log(format!("{name} 背包總數 {}，預估 {}（差 {}）", comma(tv), comma(est), comma_signed(tv - est)));
                }
            }
            let miss = est - tv;
            if bag {
                if self.running && miss > 0 {
                    let prev_t = self.total[k].unwrap().3;
                    let mut n = if miss as f64 <= (15f64).max((now - prev_t) / 60.0 * 90.0) { miss } else { 0 };
                    n = 0.max(n.min(pround(n as f64 * (self.active() - self.total_act[k]) / (now - prev_t).max(1.0))));
                    self.bag_used[k] += n;
                    self.last_drop[k] = Some(Drop { miss, t: now, n, credit: Some(0), slots: Some(slots) });
                    if n == 0 {
                        self.log(format!("{name} 背包少了 {}，不像是喝掉的，不算用量", comma(miss)));
                    }
                }
            } else {
                let n = if miss > 0 { self.qs_miss(k, miss, now, slots) } else { Some(0) };
                if self.counting() {
                    if amt > 5 {
                        self.reconcile(k, 0, now, slots);
                        if self.recon_acc[k] > 0 {
                            self.recon_acc[k] = (self.recon_acc[k] - amt).max(0);
                        }
                    } else {
                        let m = if n == Some(-1) || miss <= 0 { miss } else { 0 };
                        self.reconcile(k, m, now, slots);
                    }
                }
            }
        } else {
            self.log(format!("{name} 背包總數 {}（第一次讀到，當作起點）", comma(tv)));
        }
        self.total[k] = Some((tv, self.qs_all[k], self.qs_add[k], now));
        self.total_act[k] = self.active();
        self.qs_sus[k] = 0;
        self.qs_sus_cnt[k] = 0;
    }

    pub fn feed_inv(&mut self, k: usize, stacks: &[Option<i64>]) -> &'static str {
        let cap = self.cfg_f("stack_max") as i64;
        if stacks.iter().any(|v| v.map_or(true, |v| cap != 0 && v > cap)) {
            return "bad";
        }
        let tv: i64 = stacks.iter().map(|v| v.unwrap()).sum();
        let n = stacks.len() as i64;
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
        let need = match self.est_total(k) {
            None => 3,
            Some(e) if tv > e + 5 => 4,
            Some(e) if (e - tv) as f64 > (60f64).max((self.now() - self.total[k].unwrap().3) / 60.0 * 40.0) => {
                if self.inv_slots[k].map_or(false, |s| s != 0 && n < s) && e - tv > 200 { 6 } else { 4 }
            }
            _ => 2,
        };
        if cnt < need {
            return "wait";
        }
        let slots = (self.inv_slots[k], n);
        self.inv_slots[k] = Some(n);
        self.set_total(k, tv, Some(slots));
        "ok"
    }

    pub fn forget_potion(&mut self, k: usize) {
        self.total[k] = None;
        self.inv_pend[k] = None;
        self.inv_slots[k] = None;
        self.last_rise[k] = None;
        self.last_drop[k] = None;
        self.recon_acc[k] = 0;
        self.recon_t[k] = None;
        self.qs_credit[k] = 0;
        self.qs_sus[k] = 0;
    }

    pub fn forget_qs(&mut self, k: usize) {
        self.qs[k] = None;
        self.qs_pend[k] = None;
        self.qs_rise[k] = None;
        self.cap_since[k] = None;
    }

    pub fn switch_potion(&mut self, k: usize, name: &str, price: f64, mark: Option<i64>) {
        let old = self.name(k);
        let mark = mark.unwrap_or_else(|| self.used(k)).min(self.used(k));
        let n = (mark - self.used_mark[k]).max(0);
        self.cost_prev[k] += n as f64 * self.price(k);
        self.prev_price[k] = Some(self.price(k));
        self.used_mark[k] = mark;
        let num = |v: f64| -> Value { if v == v.trunc() { json!(v as i64) } else { json!(v) } };
        self.cfg.insert(format!("{}_name", KS[k]), json!(name));
        self.cfg.insert(format!("{}_price", KS[k]), num(price));
        self.forget_potion(k);
        self.forget_qs(k);
        if n > 0 {
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
        let cost = self.cost_of(0) + self.cost_of(1);
        s.cost = cost;
        s.cost_hr = per(cost);
        s.eff = if cost > 0.0 { Some(self.gain as f64 / cost * 1e4) } else { None };
        s.hp_hr = per(self.used(0) as f64);
        s.mp_hr = per(self.used(1) as f64);
        s.income = self.income();
        s.profit = s.income.map(|i| i as f64 - cost);
        s.income_hr = s.income.and_then(|i| per(i as f64));
        s.profit_hr = s.profit.and_then(per);
        s
    }

    pub fn tick(&mut self) -> Stats {
        self.check_idle();
        let s = self.stats();
        let a = s.active;
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
            self.recon_acc = [0; 2];
            self.recon_t = [None; 2];
            self.seg_t = now;
            let a = self.active();
            for k in 0..2 {
                self.total_act[k] = self.total_act[k].min(a);
            }
            self.start_meso();
            for k in 0..2 {
                if let Some(t) = self.total[k] {
                    if !self.is_qs_mode(k) && now - t.3 > 600.0 {
                        self.total[k] = None;
                    }
                }
            }
            let need_exp = !self.has("exp_region") && self.exp_last.is_none();
            let no_pot = (0..2).any(|k| self.est_total(k).is_none() && !self.region(k));
            let need_meso = self.has("meso_region") && self.meso_anchor.is_none();
            self.say("開始計時", 4.0);
            self.log("===== 開始計時 =====".into());
            if (no_pot || need_meso) && self.inv_ready(None) && !need_exp {
                self.say("開始計時，請打開背包，讓浮窗讀藥水總數和楓幣", 8.0);
            } else if need_exp || no_pot {
                self.say("開始計時，先填入目前的數值當起點", 6.0);
                self.ui.push(Ui::OpenRecord);
            }
        } else if self.paused {
            self.paused = false;
            self.run_start = now;
            self.exp_move_t = now;
            self.cap_since = [None; 2];
            let no_end = self.seg_pending;
            if no_end {
                self.seg_pending = false;
            } else {
                self.start_meso();
            }
            self.log("繼續計時".into());
            self.recon_acc = [0; 2];
            self.recon_t = [None; 2];
            self.seg_t = now;
            if no_end && self.has("meso_region") && self.meso_anchor.is_some() {
                self.say("繼續計時（暫停期間沒記錄到楓幣，這段的楓幣增減會計算）", 8.0);
            } else if self.has("meso_region") && self.meso_anchor.is_none() && self.inc_done.is_some() {
                self.say("繼續計時，請打開背包確認楓幣", 8.0);
            } else {
                self.say("繼續計時", 4.0);
            }
        } else {
            self.acc += now - self.run_start - back;
            self.paused = true;
            if back != 0.0 {
                let acc = self.acc;
                self.series.retain(|x| x.0 <= acc);
                if let Some(ma) = self.meso_anchor {
                    if ma.1 > acc {
                        self.meso_anchor = Some((ma.0, acc, ma.2));
                    }
                }
                for k in 0..2 {
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
            for k in 0..2 {
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
        for k in 0..2 {
            let Some(text) = &out.qs[k] else { continue };
            self.raw.insert(KS[k].into(), text.clone());
            let cap = self.cfg_f("stack_max") as i64;
            let v = parse_count(text).filter(|&x| !(cap != 0 && x > cap));
            if out.qs_err[k].is_some() {
                parts.push(format!("{} 截圖失敗", self.name(k)));
            } else if v.is_none() {
                parts.push(format!("{} 讀不到", self.name(k)));
            } else if game_visible {
                self.feed_qs(k, v.unwrap());
            }
            if cap != 0 && self.counting() && self.qs[k] == Some(cap) {
                self.cap_since[k] = Some(self.cap_since[k].unwrap_or_else(|| self.now()));
            } else {
                self.cap_since[k] = None;
            }
        }
        let mut results: Vec<&'static str> = vec![];
        if let Some(inv) = &out.inv {
            let mut info: Vec<String> = vec![];
            let bag_open = inv.iter().any(|x| x.as_ref().map_or(false, |v| !v.is_empty()));
            for k in 0..2 {
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
                if out.meso_box_only && !self.meso_near(v) {
                    if self.meso_rej != Some(v) {
                        self.meso_rej = Some(v);
                        let tail = self.meso.map_or("還沒讀過楓幣".to_string(), |l| format!("跟上次的 {} 差太多", comma(l)));
                        self.log(format!("楓幣讀到 {}，{tail}，當成讀錯（背包裡看到藥水時再確認）", comma(v)));
                    }
                } else {
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
        let per = |f: &dyn Fn(usize) -> Value| json!({"hp": f(0), "mp": f(1)});
        match key {
            "gain" => json!(self.gain),
            "hist" => json!(self.hist.iter().map(|h| json!([h.0, h.1])).collect::<Vec<_>>()),
            "levelups" => json!(self.levelups),
            "deaths" => json!(self.deaths),
            "qs_used" => per(&|k| json!(self.qs_used[k])),
            "bag_used" => per(&|k| json!(self.bag_used[k])),
            "meso_anchor" => self.meso_anchor.map_or(Value::Null, |m| json!([m.0, m.1, m.2])),
            "series" => json!(self.series.iter().map(|s| json!([s.0, s.1, s.2, s.3, s.4])).collect::<Vec<_>>()),
            "qs_sus_cnt" => per(&|k| json!(self.qs_sus_cnt[k])),
            "cost_prev" => per(&|k| json!(self.cost_prev[k])),
            "prev_price" => per(&|k| json!(self.prev_price[k])),
            "used_mark" => per(&|k| json!(self.used_mark[k])),
            "last_drop" => per(&|k| match &self.last_drop[k] {
                None => Value::Null,
                Some(d) => {
                    let mut v = vec![json!(d.miss), json!(d.t), json!(d.n)];
                    if let Some(c) = d.credit {
                        v.push(json!(c));
                    }
                    if let Some(s) = d.slots {
                        v.push(s.map_or(Value::Null, |s| json!([s.0, s.1])));
                    }
                    Value::Array(v)
                }
            }),
            "inc_done" => self.inc_done.map_or(Value::Null, |d| json!([d.0, d.1])),
            "seg_pending" => json!(self.seg_pending),
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
            json!(self.series.iter().filter(|x| x.0 > a - 3900.0).map(|s| json!([s.0, s.1, s.2, s.3, s.4])).collect::<Vec<_>>()),
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
                    a.iter().filter_map(|x| Some((f(&x[0])?, i(&x[1])?, i(&x[2])?, i(&x[3])?, i(&x[4])))).collect()
                })
            }
            "seg_pending" => self.seg_pending = v.as_bool().unwrap_or(false),
            "need" => self.need = f(v),
            "meso" => self.meso = i(v),
            "meso_t" => self.meso_t = f(v).unwrap_or(0.0),
            "inc_done" => self.inc_done = v.as_array().map(|a| (i(&a[0]).unwrap_or(0), a.last().and_then(f).unwrap_or(0.0))),
            _ => {
                for k in 0..2 {
                    let x = v.get(KS[k]).cloned().unwrap_or(Value::Null);
                    match key {
                        "qs_used" => self.qs_used[k] = i(&x).unwrap_or(0),
                        "bag_used" => self.bag_used[k] = i(&x).unwrap_or(0),
                        "qs_sus_cnt" => self.qs_sus_cnt[k] = i(&x).unwrap_or(0),
                        "cost_prev" => self.cost_prev[k] = f(&x).unwrap_or(0.0),
                        "prev_price" => self.prev_price[k] = f(&x),
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
                            self.last_drop[k] = x.as_array().and_then(|a| {
                                Some(Drop {
                                    miss: i(a.first()?)?,
                                    t: f(a.get(1)?)?,
                                    n: i(a.get(2)?)?,
                                    credit: a.get(3).and_then(i),
                                    slots: a.get(4).map(|s| s.as_array().map(|s| (i(&s[0]), i(&s[1]).unwrap_or(0)))),
                                })
                            })
                        }
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
            for k in 0..2 {
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

    pub fn log_session(&mut self) -> Saved {
        let a = self.active();
        if a < 60.0 {
            return Saved::TooShort;
        }
        let Some(path) = self.log_path.clone() else { return Saved::Main };
        let s = self.stats();
        let header: Vec<String> = [
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
        ]
        .to_vec();
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
        ];
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let ext = path.extension().map_or("csv".to_string(), |e| e.to_string_lossy().to_string());
        let alt = path.with_file_name(format!("{stem}_{}.{ext}", local_stamp()));
        for (i, p) in [path, alt].iter().enumerate() {
            if write_csv_row(p, &header, &row).is_ok() {
                return if i == 0 { Saved::Main } else { Saved::Other(p.file_name().unwrap().to_string_lossy().into()) };
            }
        }
        Saved::Failed
    }
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
