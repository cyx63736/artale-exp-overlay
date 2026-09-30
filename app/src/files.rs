use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Paths {
    pub here: PathBuf,
    pub cfg: PathBuf,
    pub state: PathBuf,
    pub csv: PathBuf,
    pub detail: PathBuf,
    pub err: PathBuf,
    pub tpl: [PathBuf; 2],
    pub digits: PathBuf,
    pub models: PathBuf,
}

impl Paths {
    pub fn new() -> Paths {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        let here = exe.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        let models = [here.join("models"), here.join("..").join("..").join("..").join("models")]
            .into_iter()
            .find(|p| p.join("keys.txt").exists())
            .unwrap_or_else(|| here.join("models"));
        Paths {
            cfg: here.join("config.json"),
            state: here.join("session.json"),
            csv: here.join("練功紀錄.csv"),
            detail: here.join("明細紀錄.log"),
            err: here.join("error.log"),
            tpl: [here.join("tpl_hp.png"), here.join("tpl_mp.png")],
            digits: here.join("digits.json"),
            models,
            here,
        }
    }
}

pub fn log_detail(p: &Path, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    if std::fs::metadata(p).map_or(false, |m| m.len() > 2_000_000) {
        let _ = std::fs::rename(p, p.with_extension("log.old"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
        let stamp = tracker::log_stamp();
        for l in lines {
            let _ = writeln!(f, "{stamp}  {l}");
        }
    }
}

pub fn log_error(p: &Path, text: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
        let (y, mo, d, h, mi, s, _) = tracker::local_now();
        let _ = writeln!(f, "{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}\n{text}\n");
    }
}

pub fn load_cfg(p: &Path) -> serde_json::Map<String, serde_json::Value> {
    let mut cfg = tracker::default_cfg();
    let mut named = false;
    if let Ok(s) = std::fs::read_to_string(p) {
        if let Ok(serde_json::Value::Object(m)) = serde_json::from_str(&s) {
            named = m.contains_key("hp_name");
            for (k, v) in m {
                cfg.insert(k, v);
            }
        }
    }
    if !cfg.contains_key("names_ok") {
        cfg.insert("names_ok".into(), named.into());
    }
    for k in ["remind_min", "sound"] {
        cfg.remove(k);
    }
    cfg.insert("stack_max".into(), 3000.into());
    cfg
}

pub fn remove_old_runtime(here: &Path) -> bool {
    let d = here.join("_internal");
    if d.join("base_library.zip").is_file() && d.join("rapidocr_onnxruntime").is_dir() {
        return std::fs::remove_dir_all(&d).is_ok();
    }
    false
}

pub fn save_cfg(p: &Path, cfg: &serde_json::Map<String, serde_json::Value>) -> bool {
    match serde_json::to_string_pretty(cfg) {
        Ok(s) => std::fs::write(p, s).is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_config_has_unconfirmed_names() {
        let dir = std::env::temp_dir().join("names_ok_test");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("config.json");
        let _ = std::fs::remove_file(&p);
        assert_eq!(load_cfg(&p)["names_ok"], serde_json::json!(false));
        std::fs::write(&p, r#"{"hp_name": "沙嗲"}"#).unwrap();
        assert_eq!(load_cfg(&p)["names_ok"], serde_json::json!(true));
        std::fs::write(&p, r#"{"hp_name": "馴鹿奶", "names_ok": false}"#).unwrap();
        assert_eq!(load_cfg(&p)["names_ok"], serde_json::json!(false));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod old_runtime_tests {
    use super::*;

    #[test]
    fn removes_only_the_old_runtime_folder() {
        let here = std::env::temp_dir().join("old_runtime_test");
        let _ = std::fs::remove_dir_all(&here);
        std::fs::create_dir_all(here.join("_internal").join("rapidocr_onnxruntime")).unwrap();
        assert!(!remove_old_runtime(&here));
        std::fs::write(here.join("_internal").join("base_library.zip"), b"x").unwrap();
        std::fs::write(here.join("config.json"), b"{}").unwrap();
        assert!(remove_old_runtime(&here));
        assert!(!here.join("_internal").exists() && here.join("config.json").exists());
        let _ = std::fs::remove_dir_all(&here);
    }
}
