use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const KIND: [&str; 2] = ["hp", "mp"];
const QS_VERSION: i64 = 2;

#[derive(Clone, Debug)]
pub struct Potion {
    pub kind: usize,
    pub name: String,
    pub price: f64,
    pub tpl: Option<String>,
    pub qs: Option<Vec<f32>>,
}

pub struct Book {
    pub list: Vec<Potion>,
    path: PathBuf,
    dir: PathBuf,
}

fn num(v: f64) -> Value {
    if v == v.trunc() {
        json!(v as i64)
    } else {
        json!(v)
    }
}

impl Book {
    pub fn load(here: &Path) -> Book {
        let path = here.join("potions.json");
        let dir = here.join("potions");
        let mut list = vec![];
        if let Ok(s) = std::fs::read_to_string(&path) {
            if let Ok(Value::Array(a)) = serde_json::from_str::<Value>(&s) {
                for v in a {
                    let kind = match v.get("kind").and_then(|x| x.as_str()) {
                        Some("mp") => 1,
                        Some("hp") => 0,
                        _ => continue,
                    };
                    let Some(name) = v.get("name").and_then(|x| x.as_str()).filter(|n| !n.trim().is_empty()) else { continue };
                    list.push(Potion {
                        kind,
                        name: name.to_string(),
                        price: v.get("price").and_then(|x| x.as_f64()).unwrap_or(0.0),
                        tpl: v.get("tpl").and_then(|x| x.as_str()).map(|s| s.to_string()),
                        qs: v
                            .get("qs")
                            .filter(|_| v.get("qsv").and_then(|x| x.as_i64()) == Some(QS_VERSION))
                            .and_then(|x| x.as_array())
                            .map(|a| a.iter().filter_map(|f| f.as_f64().map(|f| f as f32)).collect()),
                    });
                }
            }
        }
        Book { list, path, dir }
    }

    pub fn save(&self) -> bool {
        let a: Vec<Value> = self
            .list
            .iter()
            .map(|p| {
                json!({
                    "kind": KIND[p.kind],
                    "name": p.name,
                    "price": num(p.price),
                    "tpl": p.tpl,
                    "qs": p.qs.as_ref().map(|q| q.iter().map(|f| (*f as f64 * 1e4).round() / 1e4).collect::<Vec<_>>()),
                    "qsv": QS_VERSION,
                })
            })
            .collect();
        let tmp = self.path.with_extension("json.tmp");
        match serde_json::to_string_pretty(&a) {
            Ok(s) => std::fs::write(&tmp, s).and_then(|_| std::fs::rename(&tmp, &self.path)).is_ok(),
            Err(_) => false,
        }
    }

    pub fn find(&self, k: usize, name: &str) -> Option<&Potion> {
        self.list.iter().find(|p| p.kind == k && p.name == name)
    }

    fn find_mut(&mut self, k: usize, name: &str) -> Option<&mut Potion> {
        self.list.iter_mut().find(|p| p.kind == k && p.name == name)
    }

    pub fn of(&self, k: usize) -> Vec<&Potion> {
        self.list.iter().filter(|p| p.kind == k).collect()
    }

    pub fn upsert(&mut self, k: usize, name: &str, price: f64) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        match self.find_mut(k, name) {
            Some(p) => p.price = price,
            None => self.list.push(Potion { kind: k, name: name.to_string(), price, tpl: None, qs: None }),
        }
    }

    pub fn remove(&mut self, k: usize, name: &str) {
        if let Some(i) = self.list.iter().position(|p| p.kind == k && p.name == name) {
            let p = self.list.remove(i);
            if let Some(f) = p.tpl {
                let _ = std::fs::remove_file(self.dir.join(f));
            }
        }
    }

    pub fn store_tpl(&mut self, k: usize, name: &str, src: &Path) -> bool {
        let Ok(bytes) = std::fs::read(src) else { return false };
        if self.find(k, name).is_none() {
            return false;
        }
        let _ = std::fs::create_dir_all(&self.dir);
        let file = match self.find(k, name).and_then(|p| p.tpl.clone()) {
            Some(f) => f,
            None => {
                let n = (1..).find(|n| !self.list.iter().any(|p| p.tpl.as_deref() == Some(&format!("{}_{n}.png", KIND[k])))).unwrap();
                format!("{}_{n}.png", KIND[k])
            }
        };
        if std::fs::write(self.dir.join(&file), bytes).is_err() {
            return false;
        }
        self.find_mut(k, name).unwrap().tpl = Some(file);
        true
    }

    pub fn restore_tpl(&self, k: usize, name: &str, dst: &Path) -> bool {
        if let Some(bytes) = self.find(k, name).and_then(|p| p.tpl.as_ref()).and_then(|f| std::fs::read(self.dir.join(f)).ok()) {
            if std::fs::write(dst, bytes).is_ok() {
                return true;
            }
        }
        let _ = std::fs::remove_file(dst);
        false
    }

    pub fn move_look(&mut self, k: usize, from: &str, to: &str) {
        let Some(i) = self.list.iter().position(|p| p.kind == k && p.name == from) else { return };
        let (tpl, qs) = (self.list[i].tpl.take(), self.list[i].qs.take());
        let dir = self.dir.clone();
        let Some(p) = self.find_mut(k, to) else { return };
        if let Some(t) = tpl {
            if let Some(old) = p.tpl.replace(t) {
                let _ = std::fs::remove_file(dir.join(old));
            }
        }
        if qs.is_some() {
            p.qs = qs;
        }
    }

    pub fn drop_tpl(&mut self, k: usize, name: &str) {
        let dir = self.dir.clone();
        if let Some(p) = self.find_mut(k, name) {
            if let Some(f) = p.tpl.take() {
                let _ = std::fs::remove_file(dir.join(f));
            }
        }
    }

    pub fn set_qs(&mut self, k: usize, name: &str, sig: Option<Vec<f32>>) {
        if let Some(p) = self.find_mut(k, name) {
            p.qs = sig;
        }
    }

    pub fn drop_similar_qs(&mut self, k: usize, name: &str, sig: &[f32]) {
        for p in self.list.iter_mut().filter(|p| p.kind == k && p.name != name) {
            if p.qs.as_ref().map_or(false, |q| diff(q, sig) < SAME) {
                p.qs = None;
            }
        }
    }

    pub fn clear_qs(&mut self, k: usize) {
        for p in self.list.iter_mut().filter(|p| p.kind == k) {
            p.qs = None;
        }
    }
}

fn amount(a: &[f32]) -> f32 {
    a.iter().sum()
}

pub fn strong(a: &[f32]) -> bool {
    amount(a) >= 0.03
}

pub fn diff(a: &[f32], b: &[f32]) -> f32 {
    let d: f32 = a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum();
    d / amount(a).max(amount(b)).max(0.02)
}

pub fn kept(now: &[f32], before: &[f32]) -> f32 {
    let k: f32 = now.iter().zip(before).map(|(a, b)| a.min(*b)).sum();
    k / amount(before).max(1e-6)
}

pub const SAME: f32 = 0.25;
pub const CHANGED: f32 = 0.5;

pub fn flooded(a: &[f32]) -> bool {
    amount(a) > 0.55
}

pub fn swapped(now: &[f32], before: &[f32]) -> bool {
    diff(now, before) >= CHANGED && (!strong(before) || kept(now, before) < 0.5)
}

#[derive(Clone, Debug)]
pub struct Undo {
    pub name: String,
    pub price: Value,
    pub cost_prev: f64,
    pub used_mark: i64,
    pub prev_price: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct Check {
    pub t0: f64,
    pub from_sig: Option<Vec<f32>>,
    pub v0: Option<i64>,
    pub same: u32,
    pub changed: u32,
    pub undo: Undo,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Prompt {
    GameChanged { mark: i64, suggest: Option<String> },
    Mismatch { from: String },
}

#[derive(Default)]
pub struct Watch {
    pub reference: Option<Vec<f32>>,
    learn: Vec<(f64, Vec<f32>)>,
    diff_since: Option<(f64, i64, u32)>,
    odd_since: Option<(f64, u32)>,
    last_val: Option<i64>,
    jumped_at: Option<f64>,
    pub check: Option<Check>,
    pub prompt: Option<Prompt>,
    pub mark_hint: Option<(i64, f64)>,
    pub dirty: bool,
    last_store: f64,
    trust: bool,
    pub last_sig: Option<Vec<f32>>,
    pub claimed: bool,
}

pub struct Seen<'a> {
    pub now: f64,
    pub sig: &'a [f32],
    pub val: i64,
    pub cells: Option<&'a [Option<i64>]>,
    pub used: i64,
    pub effect: bool,
}

pub enum Event {
    None,
    Confirmed,
}

impl Watch {
    pub fn reset(&mut self, reference: Option<Vec<f32>>) {
        self.reference = reference;
        self.learn.clear();
        self.diff_since = None;
        self.odd_since = None;
        self.dirty = false;
        self.trust = false;
    }

    pub fn trust_screen(&mut self) {
        self.reset(None);
        self.trust = true;
    }

    pub fn feed(&mut self, s: &Seen, book: &[(String, Option<Vec<f32>>)]) -> Event {
        if let Some(p) = self.last_val {
            if (s.val - p).abs() > 60 && p > 60 {
                self.jumped_at = Some(s.now);
            }
        }
        self.last_val = Some(s.val);
        if !s.effect {
            self.last_sig = Some(s.sig.to_vec());
        }
        if let Some(c) = &mut self.check {
            let e = Self::feed_check(c, &mut self.prompt, &mut self.learn, s);
            if matches!(e, Event::Confirmed) && self.reference.as_ref().map_or(false, |r| diff(s.sig, r) >= SAME) {
                self.trust_screen();
            }
            return e;
        }
        if s.effect {
            self.diff_since = None;
            return Event::None;
        }
        let Some(r) = self.reference.clone() else {
            self.learn.push((s.now, s.sig.to_vec()));
            self.learn.retain(|(t, _)| s.now - t <= 30.0);
            let n = self.learn.len();
            if n >= 5 && s.now - self.learn[0].0 >= 10.0 {
                let mut mean = vec![0f32; s.sig.len()];
                for (_, v) in &self.learn {
                    for (m, x) in mean.iter_mut().zip(v) {
                        *m += x / n as f32;
                    }
                }
                if self.learn.iter().all(|(_, v)| diff(v, &mean) < SAME) {
                    self.learn.clear();
                    let other = (if self.trust { &[][..] } else { book })
                        .iter()
                        .filter_map(|(name, q)| q.as_ref().filter(|q| strong(q) || strong(&mean)).map(|q| (name, diff(&mean, q))))
                        .filter(|(_, d)| *d < SAME)
                        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                        .map(|(n, _)| n.clone());
                    if let Some(o) = other {
                        if self.prompt.is_none() {
                            self.prompt = Some(Prompt::GameChanged { mark: s.used, suggest: Some(o) });
                        }
                        return Event::None;
                    }
                    self.reference = Some(mean);
                    self.dirty = true;
                    self.claimed = self.trust;
                    self.trust = false;
                    self.last_store = s.now;
                }
            }
            return Event::None;
        };
        if !strong(&r) && !strong(s.sig) {
            return Event::None;
        }
        let d = diff(s.sig, &r);
        if d >= SAME && !swapped(s.sig, &r) {
            let (t0, n) = self.odd_since.get_or_insert((s.now, 0));
            *n += 1;
            if s.now - *t0 >= 180.0 && *n >= 5 {
                self.reset(None);
            }
            return Event::None;
        }
        self.odd_since = None;
        if swapped(s.sig, &r) {
            let (t0, mark, n) = self.diff_since.get_or_insert((s.now, s.used, 0));
            *n += 1;
            let jumped = self.jumped_at.map_or(false, |t| t >= *t0 - 15.0);
            let wait = if jumped { 12.0 } else { 180.0 };
            if s.now - *t0 >= wait && *n >= 3 && self.prompt.is_none() {
                let suggest = book
                    .iter()
                    .filter_map(|(name, q)| q.as_ref().map(|q| (name, diff(s.sig, q))))
                    .filter(|(_, d)| *d < SAME)
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                    .map(|(n, _)| n.clone());
                self.prompt = Some(Prompt::GameChanged { mark: *mark, suggest });
            }
        } else if d < SAME {
            self.diff_since = None;
            if matches!(self.prompt, Some(Prompt::GameChanged { .. })) {
                self.prompt = None;
            }
            let r = self.reference.as_mut().unwrap();
            for (a, b) in r.iter_mut().zip(s.sig) {
                *a = *a * 0.95 + b * 0.05;
            }
            if s.now - self.last_store > 300.0 {
                self.last_store = s.now;
                self.dirty = true;
            }
        }
        Event::None
    }

    fn feed_check(c: &mut Check, prompt: &mut Option<Prompt>, learn: &mut Vec<(f64, Vec<f32>)>, s: &Seen) -> Event {
        let from = c.from_sig.as_ref().filter(|f| !s.effect && (strong(f) || strong(s.sig)));
        let sig_same = from.map_or(false, |f| diff(s.sig, f) < SAME);
        let sig_changed = from.map_or(false, |f| swapped(s.sig, f));
        let by_val = c.v0.map(|v0| s.val <= v0 + 1 && s.val >= v0 - 60);
        let by_bag = s.cells.filter(|_| s.val % 100 != 0).map(|cells| {
            let hit = cells.iter().flatten().any(|&n| (n - s.val).abs() <= 2);
            if hit {
                Some(false)
            } else if cells.is_empty() {
                Some(true)
            } else {
                None
            }
        });
        let changed = sig_changed || by_val == Some(false) || by_bag == Some(Some(false));
        let same = !changed && (sig_same || (!sig_same && !sig_changed && by_val == Some(true)) || by_bag == Some(Some(true)));
        if changed {
            c.changed += 1;
            c.same = 0;
        } else if same {
            c.same += 1;
            c.changed = 0;
        }
        if c.changed >= 2 {
            learn.clear();
            *prompt = None;
            return Event::Confirmed;
        }
        if c.same >= 3 && s.now - c.t0 >= 8.0 && prompt.is_none() {
            *prompt = Some(Prompt::Mismatch { from: c.undo.name.clone() });
        }
        Event::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SATAY: [f32; 12] = [0.036, 0.032, 0.035, 0.002, 0.002, 0.029, 0.007, 0.0, 0.0, 0.006, 0.036, 0.005];
    const ICE: [f32; 12] = [0.0, 0.003, 0.001, 0.0, 0.0, 0.0, 0.047, 0.004, 0.0, 0.0, 0.0, 0.0];
    const MILK: [f32; 12] = [0.09, 0.152, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

    fn seen<'a>(now: f64, sig: &'a [f32], val: i64) -> Seen<'a> {
        Seen { now, sig, val, cells: None, used: 100, effect: false }
    }

    fn undo() -> Undo {
        Undo { name: "沙嗲".into(), price: json!(2600), cost_prev: 0.0, used_mark: 0, prev_price: None }
    }

    #[test]
    fn book_remembers_bag_cells() {
        let dir = std::env::temp_dir().join(format!("potbook_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let tpl = dir.join("tpl_hp.png");
        let mut b = Book::load(&dir);
        b.upsert(0, "沙嗲", 2600.0);
        b.upsert(0, "馴鹿奶", 5600.0);
        std::fs::write(&tpl, b"satay").unwrap();
        assert!(b.store_tpl(0, "沙嗲", &tpl));
        std::fs::write(&tpl, b"milk").unwrap();
        assert!(b.store_tpl(0, "馴鹿奶", &tpl));
        b.set_qs(0, "沙嗲", Some(SATAY.to_vec()));
        assert!(b.save());
        let b = Book::load(&dir);
        assert_eq!(b.of(0).len(), 2);
        assert!(b.restore_tpl(0, "沙嗲", &tpl));
        assert_eq!(std::fs::read(&tpl).unwrap(), b"satay");
        assert!(b.find(0, "沙嗲").unwrap().qs.is_some());
        let mut b = b;
        b.upsert(0, "白水", 100.0);
        assert!(!b.restore_tpl(0, "白水", &tpl));
        assert!(!tpl.exists());
        b.remove(0, "馴鹿奶");
        assert!(b.find(0, "馴鹿奶").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn effects_on_top_are_not_a_swap() {
        let mut gold = ICE;
        gold[1] += 0.05;
        gold[2] += 0.03;
        assert!(diff(&gold, &ICE) >= CHANGED);
        assert!(!swapped(&gold, &ICE));
        assert!(swapped(&SATAY, &ICE) && swapped(&MILK, &SATAY) && swapped(&ICE, &SATAY));
        let mut w = Watch::default();
        w.reset(Some(ICE.to_vec()));
        for i in 0..10 {
            w.feed(&seen(i as f64 * 3.0, &gold, 2900), &[]);
        }
        assert!(w.prompt.is_none());
    }

    const WRONG: [f32; 12] = [0.001, 0.008, 0.0, 0.0, 0.0, 0.0, 0.028, 0.013, 0.0, 0.0, 0.0, 0.0];
    const LIVE: [f32; 12] = [0.037, 0.046, 0.036, 0.001, 0.0, 0.0, 0.054, 0.0, 0.0, 0.0, 0.047, 0.0];

    #[test]
    fn wrong_reference_is_relearned() {
        assert!(diff(&LIVE, &WRONG) >= SAME && !swapped(&LIVE, &WRONG));
        let mut w = Watch::default();
        w.reset(Some(WRONG.to_vec()));
        for i in 0..75 {
            w.feed(&seen(i as f64 * 3.0, &LIVE, 2896), &[]);
        }
        assert!(w.prompt.is_none());
        let r = w.reference.clone().expect("重新學好");
        assert!(diff(&r, &LIVE) < 0.05);
    }

    #[test]
    fn unsure_colors_fall_back_to_the_number() {
        let mut w = Watch::default();
        w.check = Some(Check { t0: 0.0, from_sig: Some(WRONG.to_vec()), v0: Some(2896), same: 0, changed: 0, undo: undo() });
        for i in 0..12 {
            w.feed(&seen(i as f64, &LIVE, 2896 - i / 4), &[]);
        }
        assert_eq!(w.prompt, Some(Prompt::Mismatch { from: "沙嗲".into() }));
    }

    #[test]
    fn learning_another_potions_look_prompts() {
        let mut w = Watch::default();
        let book = vec![("沙嗲".to_string(), Some(SATAY.to_vec()))];
        for i in 0..15 {
            w.feed(&seen(i as f64, &SATAY, 2896), &book);
        }
        assert!(w.reference.is_none() && !w.dirty);
        assert_eq!(w.prompt, Some(Prompt::GameChanged { mark: 100, suggest: Some("沙嗲".into()) }));
    }

    #[test]
    fn wrong_name_moves_the_look() {
        let dir = std::env::temp_dir().join("move_look_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let tpl = dir.join("tpl_hp.png");
        std::fs::write(&tpl, b"satay").unwrap();
        let mut b = Book::load(&dir);
        b.upsert(0, "馴鹿奶", 5600.0);
        b.store_tpl(0, "馴鹿奶", &tpl);
        b.set_qs(0, "馴鹿奶", Some(SATAY.to_vec()));
        b.upsert(0, "沙嗲", 2600.0);
        b.move_look(0, "馴鹿奶", "沙嗲");
        let (m, s) = (b.find(0, "馴鹿奶").unwrap(), b.find(0, "沙嗲").unwrap());
        assert!(m.tpl.is_none() && m.qs.is_none());
        assert!(s.qs.is_some());
        assert!(b.restore_tpl(0, "沙嗲", &tpl));
        assert_eq!(std::fs::read(&tpl).unwrap(), b"satay");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_answer_wins_over_a_wrong_book_entry() {
        let mut w = Watch::default();
        let book = vec![("馴鹿奶".to_string(), Some(SATAY.to_vec()))];
        w.trust_screen();
        for i in 0..15 {
            w.feed(&seen(i as f64, &SATAY, 2857), &book);
        }
        assert!(w.prompt.is_none());
        assert!(w.dirty && w.claimed);
        assert!(diff(w.reference.as_ref().unwrap(), &SATAY) < SAME);
    }

    #[test]
    fn confirmed_swap_relearns_a_wrong_reference() {
        let mut w = Watch::default();
        w.reset(Some(SATAY.to_vec()));
        w.check = Some(Check { t0: 0.0, from_sig: Some(SATAY.to_vec()), v0: Some(2896), same: 0, changed: 0, undo: undo() });
        for i in 0..40 {
            if matches!(w.feed(&seen(i as f64, &MILK, 513), &[]), Event::Confirmed) {
                w.check = None;
            }
        }
        assert!(w.prompt.is_none());
        assert!(diff(w.reference.as_ref().unwrap(), &MILK) < 0.05);
    }

    const GOLD: [f32; 12] = [0.074, 0.75, 0.022, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.024];

    #[test]
    fn aura_is_not_a_swap() {
        assert!(flooded(&GOLD) && !flooded(&MILK));
        let mut w = Watch::default();
        w.reset(Some(SATAY.to_vec()));
        for i in 0..40 {
            let s = Seen { now: i as f64 * 3.0, sig: &GOLD, val: 2890, cells: None, used: 0, effect: true };
            w.feed(&s, &[]);
        }
        assert!(w.prompt.is_none());
    }

    #[test]
    fn color_change_without_number_jump_waits_longer() {
        let mut w = Watch::default();
        w.reset(Some(SATAY.to_vec()));
        for i in 0..45 {
            w.feed(&seen(i as f64 * 3.0, &MILK, 3000), &[]);
        }
        assert!(w.prompt.is_none());
        for i in 45..65 {
            w.feed(&seen(i as f64 * 3.0, &MILK, 3000), &[]);
        }
        assert!(matches!(w.prompt, Some(Prompt::GameChanged { .. })));
    }

    #[test]
    fn distances() {
        assert!(diff(&SATAY, &ICE) > 1.0 && diff(&SATAY, &MILK) > 1.0);
        let mut near = SATAY;
        near[0] += 0.004;
        near[1] -= 0.004;
        assert!(diff(&SATAY, &near) < SAME);
    }

    #[test]
    fn learns_with_fast_reads() {
        let mut w = Watch::default();
        for i in 0..12 {
            w.feed(&seen(i as f64, &SATAY, 2900), &[]);
        }
        assert!(w.reference.is_some());
    }

    #[test]
    fn learns_then_flags_swap() {
        let mut w = Watch::default();
        for i in 0..6 {
            w.feed(&seen(i as f64 * 3.0, &SATAY, 2900 - i), &[]);
        }
        assert!(w.reference.is_some() && w.dirty);
        for i in 0..3 {
            w.feed(&seen(20.0 + i as f64 * 2.0, &MILK, 2890), &[]);
        }
        w.feed(&seen(27.0, &SATAY, 2890), &[]);
        assert!(w.prompt.is_none());
        let book = vec![("馴鹿奶".to_string(), Some(MILK.to_vec())), ("白水".to_string(), None)];
        for i in 0..6 {
            w.feed(&seen(30.0 + i as f64 * 3.0, &MILK, 1500), &book);
        }
        assert_eq!(w.prompt, Some(Prompt::GameChanged { mark: 100, suggest: Some("馴鹿奶".into()) }));
        w.feed(&seen(60.0, &SATAY, 2880), &book);
        assert!(w.prompt.is_none());
    }

    #[test]
    fn settings_changed_but_quickslot_not() {
        let mut w = Watch::default();
        w.check = Some(Check { t0: 0.0, from_sig: Some(SATAY.to_vec()), v0: Some(2900), same: 0, changed: 0, undo: undo() });
        for i in 0..3 {
            assert!(matches!(w.feed(&seen(1.0 + i as f64, &SATAY, 2899 - i), &[]), Event::None));
        }
        assert!(w.prompt.is_none());
        w.feed(&seen(9.0, &SATAY, 2896), &[]);
        assert_eq!(w.prompt, Some(Prompt::Mismatch { from: "沙嗲".into() }));
        w.feed(&seen(10.0, &ICE, 2500), &[]);
        assert!(matches!(w.feed(&seen(11.0, &ICE, 2500), &[]), Event::Confirmed));
        assert!(w.prompt.is_none());
    }

    #[test]
    fn settings_changed_and_quickslot_too() {
        let mut w = Watch::default();
        w.check = Some(Check { t0: 0.0, from_sig: Some(SATAY.to_vec()), v0: Some(2900), same: 0, changed: 0, undo: undo() });
        w.feed(&seen(1.0, &MILK, 1200), &[]);
        assert!(matches!(w.feed(&seen(2.0, &MILK, 1200), &[]), Event::Confirmed));
        assert!(w.prompt.is_none());
    }

    #[test]
    fn colorless_potion_uses_the_number() {
        let white = [0.0f32; 12];
        let mut w = Watch::default();
        w.check = Some(Check { t0: 0.0, from_sig: Some(white.to_vec()), v0: Some(800), same: 0, changed: 0, undo: undo() });
        for i in 0..3 {
            w.feed(&seen(7.0 + i as f64, &white, 800 - i), &[]);
        }
        assert!(matches!(w.prompt, Some(Prompt::Mismatch { .. })));
        let mut w = Watch::default();
        w.check = Some(Check { t0: 0.0, from_sig: None, v0: Some(800), same: 0, changed: 0, undo: undo() });
        w.feed(&seen(1.0, &white, 2999), &[]);
        assert!(matches!(w.feed(&seen(2.0, &white, 2999), &[]), Event::Confirmed));
    }

    #[test]
    fn bag_confirms() {
        let mut w = Watch::default();
        w.check = Some(Check { t0: 0.0, from_sig: Some(SATAY.to_vec()), v0: Some(2900), same: 0, changed: 0, undo: undo() });
        let cells = [Some(3000), Some(2890)];
        for i in 0..2 {
            let s = Seen { now: 1.0 + i as f64, sig: &SATAY, val: 2890, cells: Some(&cells), used: 0, effect: false };
            let e = w.feed(&s, &[]);
            if i == 1 {
                assert!(matches!(e, Event::Confirmed));
            }
        }
        let mut w = Watch::default();
        w.check = Some(Check { t0: 0.0, from_sig: None, v0: None, same: 0, changed: 0, undo: undo() });
        for i in 0..3 {
            let s = Seen { now: 7.0 + i as f64, sig: &SATAY, val: 2890, cells: Some(&[]), used: 0, effect: false };
            w.feed(&s, &[]);
        }
        assert!(matches!(w.prompt, Some(Prompt::Mismatch { .. })));
    }
}
