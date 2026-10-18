use crate::worker::Msg;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub const VERSION: &str = "1.5.5";
const API: &str = "https://api.github.com/repos/cyx63736/artale-exp-overlay/releases/latest";
const FROM: &str = "https://github.com/cyx63736/artale-exp-overlay/releases/download/";
const ASSET: &str = "_win.zip";
const OLD: &str = "update-old";
const TMP: &str = "更新暫存";

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub tag: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

pub enum Event {
    Found(Release),
    CheckFailed(String),
    Installed(Result<PathBuf, String>),
}

#[derive(Clone)]
pub struct Link {
    tx: Sender<Msg>,
    wake: Arc<Mutex<Box<dyn Fn() + Send>>>,
}

impl Link {
    pub fn new(tx: Sender<Msg>, wake: impl Fn() + Send + 'static) -> Link {
        Link { tx, wake: Arc::new(Mutex::new(Box::new(wake))) }
    }

    fn send(&self, e: Event) {
        let _ = self.tx.send(Msg::Update(e));
        (self.wake.lock().unwrap())();
    }
}

pub fn enabled(cfg: &serde_json::Map<String, serde_json::Value>) -> bool {
    cfg!(windows) && cfg.get("update_check").and_then(|v| v.as_bool()).unwrap_or(false)
}

fn ver(s: &str) -> Vec<u32> {
    s.trim().trim_start_matches(['v', 'V']).split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

pub fn newer(tag: &str, cur: &str) -> bool {
    ver(tag) > ver(cur)
}

pub fn parse(json: &str) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("看不懂 GitHub 的回應：{e}"))?;
    let tag = v["tag_name"].as_str().ok_or("GitHub 的回應沒有版本號")?.to_string();
    let a = v["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|a| a["name"].as_str().is_some_and(|n| n.ends_with(ASSET))))
        .ok_or("新版沒有 Windows 的檔案")?;
    let url = a["browser_download_url"].as_str().filter(|u| u.starts_with(FROM)).ok_or("新版的下載網址不對")?.to_string();
    let size = a["size"].as_u64().filter(|&s| s > 0).ok_or("新版的檔案大小不對")?;
    let sha256 = a["digest"]
        .as_str()
        .and_then(|d| d.strip_prefix("sha256:"))
        .filter(|h| h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or("新版的檔案沒有核對碼")?
        .to_ascii_lowercase();
    Ok(Release { tag, url, size, sha256 })
}

fn sys(exe: &str) -> PathBuf {
    std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows")).join("System32").join(exe)
}

fn run<I, S>(exe: &str, args: I) -> Result<Vec<u8>, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = std::process::Command::new(sys(exe));
    cmd.args(args);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
    let out = cmd.output().map_err(|e| format!("執行不了 {exe}：{e}"))?;
    if !out.status.success() {
        return Err(format!("{exe} 失敗（{}）：{}", out.status, String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(out.stdout)
}

pub fn check() -> Result<Option<Release>, String> {
    let body = run("curl.exe", ["-fsSL", "--max-time", "20", "-H", "Accept: application/vnd.github+json", API])?;
    let r = parse(&String::from_utf8_lossy(&body))?;
    Ok(newer(&r.tag, VERSION).then_some(r))
}

pub fn check_bg(link: Link) {
    std::thread::spawn(move || match check() {
        Ok(Some(r)) => link.send(Event::Found(r)),
        Ok(None) => {}
        Err(e) => link.send(Event::CheckFailed(e)),
    });
}

pub fn install_bg(link: Link, r: Release, here: PathBuf, models: PathBuf) {
    std::thread::spawn(move || link.send(Event::Installed(install(&r, &here, &models))));
}

pub fn install(r: &Release, here: &Path, models: &Path) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("找不到程式自己的位置：{e}"))?;
    let tmp = here.join(TMP);
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| format!("建不了「{TMP}」資料夾：{e}"))?;
    let res = fetch(r, &tmp).and_then(|x| plan(&x, here, models, &exe)).and_then(|files| swap(&files));
    let _ = std::fs::remove_dir_all(&tmp);
    res.map(|_| exe)
}

fn fetch(r: &Release, tmp: &Path) -> Result<PathBuf, String> {
    let zip = tmp.join("new.zip");
    run("curl.exe", [OsStr::new("-fsSL"), OsStr::new("--max-time"), OsStr::new("600"), OsStr::new("-o"), zip.as_os_str(), OsStr::new(&r.url)])
        .map_err(|e| format!("下載失敗：{e}"))?;
    let data = std::fs::read(&zip).map_err(|e| format!("讀不了下載的檔案：{e}"))?;
    if data.len() as u64 != r.size || hex(&sha256(&data)) != r.sha256 {
        return Err("下載的檔案跟 GitHub 上的不一樣（可能沒下載完整）".into());
    }
    drop(data);
    let x = tmp.join("x");
    std::fs::create_dir_all(&x).map_err(|e| e.to_string())?;
    run("tar.exe", [OsStr::new("-xf"), zip.as_os_str(), OsStr::new("-C"), x.as_os_str()]).map_err(|e| format!("解壓失敗：{e}"))?;
    Ok(x)
}

fn plan(x: &Path, here: &Path, models: &Path, exe: &Path) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let mut v = vec![];
    let mut dirs = vec![x.to_path_buf()];
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(&d).map_err(|e| e.to_string())?.flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs.push(p);
                continue;
            }
            let Ok(rel) = p.strip_prefix(x) else { continue };
            let dest = if rel == Path::new("ExpOverlay.exe") {
                exe.to_path_buf()
            } else if let Ok(m) = rel.strip_prefix("models") {
                models.join(m)
            } else {
                here.join(rel)
            };
            v.push((p.clone(), dest));
        }
    }
    if !v.iter().any(|(_, d)| d == exe) {
        return Err("新版的檔案裡沒有 ExpOverlay.exe".into());
    }
    Ok(v)
}

fn old_of(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".");
    s.push(OLD);
    PathBuf::from(s)
}

fn swap(files: &[(PathBuf, PathBuf)]) -> Result<(), String> {
    let mut done: Vec<(&Path, Option<PathBuf>)> = vec![];
    for (src, dest) in files {
        let one = || -> std::io::Result<Option<PathBuf>> {
            if let Some(d) = dest.parent() {
                std::fs::create_dir_all(d)?;
            }
            let old = if dest.exists() {
                let o = old_of(dest);
                let _ = std::fs::remove_file(&o);
                std::fs::rename(dest, &o)?;
                Some(o)
            } else {
                None
            };
            if let Err(e) = std::fs::rename(src, dest).or_else(|_| std::fs::copy(src, dest).map(|_| ())) {
                if let Some(o) = &old {
                    let _ = std::fs::rename(o, dest);
                }
                return Err(e);
            }
            Ok(old)
        };
        match one() {
            Ok(old) => done.push((dest, old)),
            Err(e) => {
                for (d, o) in done.into_iter().rev() {
                    let _ = std::fs::remove_file(d);
                    if let Some(o) = o {
                        let _ = std::fs::rename(o, d);
                    }
                }
                return Err(format!("換不了 {}：{e}", dest.display()));
            }
        }
    }
    Ok(())
}

pub fn relaunch(exe: &Path) -> Result<(), String> {
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--replace");
    if let Some(d) = exe.parent() {
        cmd.current_dir(d);
    }
    cmd.spawn().map(|_| ()).map_err(|e| format!("開不了新版：{e}"))
}

pub fn cleanup(here: &Path, models: &Path) {
    let olds = || -> Vec<PathBuf> {
        [here, models]
            .iter()
            .filter_map(|d| std::fs::read_dir(d).ok())
            .flat_map(|r| r.flatten().map(|e| e.path()))
            .filter(|p| p.is_file() && p.extension() == Some(OsStr::new(OLD)))
            .collect()
    };
    let _ = std::fs::remove_dir_all(here.join(TMP));
    for _ in 0..10 {
        let left: Vec<PathBuf> = olds().into_iter().filter(|p| std::fs::remove_file(p).is_err()).collect();
        if left.is_empty() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
    0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
    0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let full = data.len() / 64 * 64;
    let mut tail = data[full..].to_vec();
    tail.push(0x80);
    while tail.len() % 64 != 56 {
        tail.push(0);
    }
    tail.extend_from_slice(&(data.len() as u64 * 8).to_be_bytes());
    for block in data[..full].chunks_exact(64).chain(tail.chunks_exact(64)) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([block[i * 4], block[i * 4 + 1], block[i * 4 + 2], block[i * 4 + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut s = h;
        for i in 0..64 {
            let [a, b, c, d, e, f, g, hh] = s;
            let t1 = hh.wrapping_add(e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25)).wrapping_add((e & f) ^ (!e & g)).wrapping_add(K[i]).wrapping_add(w[i]);
            let t2 = (a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22)).wrapping_add((a & b) ^ (a & c) ^ (b & c));
            s = [t1.wrapping_add(t2), a, b, c, d.wrapping_add(t1), e, f, g];
        }
        for (x, y) in h.iter_mut().zip(s) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 32];
    for (i, x) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&x.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_sha256() {
        assert_eq!(hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(hex(&sha256(&[b'a'; 1_000_000])), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
        assert_eq!(hex(&sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")), "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
    }

    #[test]
    fn update_versions() {
        assert!(newer("v1.5.5", "1.5.4"));
        assert!(newer("v1.5.10", "1.5.9"));
        assert!(newer("v1.6", "1.5.9"));
        assert!(newer("v2.0.0", "1.9.9"));
        assert!(!newer("v1.5.4", "1.5.4"));
        assert!(!newer("v1.5.3", "1.5.4"));
        assert!(!newer("v1.4.10", "1.5.0"));
    }

    #[test]
    fn update_parse() {
        let j = r#"{"tag_name":"v1.5.4","assets":[
            {"name":"artale-exp-overlay_v1.5.4_mac.zip","size":31967815,"digest":"sha256:3ea7c1efb7f211bb3740cece484c1893f589313aeeaa54d8ac13758bb7d33617","browser_download_url":"https://github.com/cyx63736/artale-exp-overlay/releases/download/v1.5.4/artale-exp-overlay_v1.5.4_mac.zip"},
            {"name":"artale-exp-overlay_v1.5.4_win.zip","size":26742346,"digest":"sha256:E1E8E6102F2A5E612CA89179627A6DA1FB446D532645C776DC738EAD235BFF8C","browser_download_url":"https://github.com/cyx63736/artale-exp-overlay/releases/download/v1.5.4/artale-exp-overlay_v1.5.4_win.zip"}]}"#;
        let r = parse(j).unwrap();
        assert_eq!(r.tag, "v1.5.4");
        assert_eq!(r.size, 26742346);
        assert_eq!(r.sha256, "e1e8e6102f2a5e612ca89179627a6da1fb446d532645c776dc738ead235bff8c");
        assert!(r.url.ends_with("_win.zip"));
        assert!(parse(&j.replace("https://github.com/cyx63736/", "https://example.com/")).is_err());
        assert!(parse(&j.replace("sha256:E1E8", "md5:E1E8")).is_err());
        assert!(parse(&j.replace("_win.zip", "_linux.zip")).is_err());
    }

    #[test]
    fn update_swap() {
        let d = std::env::temp_dir().join(format!("update_swap_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (here, x) = (d.join("app"), d.join("x"));
        let models = here.join("models");
        for (p, s) in [
            (here.join("ExpOverlay_test.exe"), "old exe"),
            (models.join("a.onnx"), "old a"),
            (here.join("config.json"), "cfg"),
            (here.join("明細紀錄.log.old"), "log"),
            (x.join("ExpOverlay.exe"), "new exe"),
            (x.join("models").join("a.onnx"), "new a"),
            (x.join("models").join("b.onnx"), "new b"),
        ] {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, s).unwrap();
        }
        let exe = here.join("ExpOverlay_test.exe");
        let files = plan(&x, &here, &models, &exe).unwrap();
        assert_eq!(files.len(), 3);
        swap(&files).unwrap();
        let rd = |p: PathBuf| std::fs::read_to_string(p).unwrap_or_default();
        assert_eq!(rd(exe.clone()), "new exe");
        assert_eq!(rd(old_of(&exe)), "old exe");
        assert_eq!(rd(models.join("a.onnx")), "new a");
        assert_eq!(rd(models.join("b.onnx")), "new b");
        assert_eq!(rd(here.join("config.json")), "cfg");
        cleanup(&here, &models);
        assert!(!old_of(&exe).exists() && !old_of(&models.join("a.onnx")).exists());
        assert_eq!(rd(here.join("明細紀錄.log.old")), "log");

        std::fs::write(x.join("ExpOverlay.exe"), "newer exe").unwrap();
        std::fs::create_dir_all(here.join("blocked")).unwrap();
        std::fs::write(x.join("blocked"), "x").unwrap();
        let files = vec![(x.join("ExpOverlay.exe"), exe.clone()), (x.join("blocked"), here.join("blocked").join("."))];
        assert!(swap(&files).is_err());
        assert_eq!(rd(exe.clone()), "new exe");
        assert!(!old_of(&exe).exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    #[ignore]
    fn update_live() {
        let body = run("curl.exe", ["-fsSL", "--max-time", "20", API]).unwrap();
        let r = parse(&String::from_utf8_lossy(&body)).unwrap();
        println!("{r:?}");
        assert!(r.tag.starts_with('v'));
    }

    #[test]
    #[ignore]
    fn update_local() {
        let zip = PathBuf::from(std::env::var("UPDATE_ZIP").unwrap());
        let sha = std::env::var("UPDATE_SHA").unwrap();
        let size = std::fs::metadata(&zip).unwrap().len();
        let url = format!("file:///{}", zip.display().to_string().replace('\\', "/"));
        let d = std::env::temp_dir().join(format!("update_local_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (here, models) = (d.join("app"), d.join("app").join("models"));
        std::fs::create_dir_all(&models).unwrap();
        std::fs::write(here.join("config.json"), "{}").unwrap();
        let tmp = here.join(TMP);
        std::fs::create_dir_all(&tmp).unwrap();
        let r = Release { tag: "v9.9.9".into(), url: url.clone(), size, sha256: sha.clone() };
        let x = fetch(&r, &tmp).unwrap();
        let exe = here.join("ExpOverlay.exe");
        std::fs::write(&exe, "old").unwrap();
        let files = plan(&x, &here, &models, &exe).unwrap();
        swap(&files).unwrap();
        assert!(std::fs::metadata(&exe).unwrap().len() > 1_000_000);
        assert!(models.join("keys.txt").exists());
        assert_eq!(std::fs::read_to_string(here.join("config.json")).unwrap(), "{}");
        let bad = Release { sha256: "0".repeat(64), ..r };
        assert!(fetch(&bad, &tmp).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
