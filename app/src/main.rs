#![windows_subsystem = "windows"]

mod dialogs;
mod files;
mod platform;
mod potions;
mod runner;
mod select;
mod ui;
mod worker;

use std::sync::mpsc::channel;

fn main() {
    platform::dpi_aware();
    let paths = files::Paths::new();
    let _guard = match platform::single_instance() {
        platform::Instance::Only(g) => g,
        platform::Instance::Quit => return,
    };
    let err_path = paths.err.clone();
    std::panic::set_hook(Box::new(move |info| {
        files::log_error(&err_path, &format!("{info}\n{}", std::backtrace::Backtrace::force_capture()));
    }));

    {
        let (here, detail) = (paths.here.clone(), paths.detail.clone());
        std::thread::spawn(move || {
            if files::remove_old_runtime(&here) {
                files::log_detail(&detail, &["刪除舊版留下的 _internal 資料夾（新版用不到）".to_string()]);
            }
        });
    }
    let cfg = files::load_cfg(&paths.cfg);
    let mut t = tracker::Tracker::new(cfg.clone(), None);
    t.state_path = Some(paths.state.clone());
    t.log_path = Some(paths.csv.clone());
    t.tpl_exists = [paths.tpl[0].exists(), paths.tpl[1].exists()];
    if let Ok(s) = std::fs::read_to_string(&paths.state) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
            t.load_state(&v);
        }
    }
    files::log_detail(&paths.detail, &std::mem::take(&mut t.logs));

    let shared = worker::Shared::new(cfg);
    let (tx, rx) = channel();
    let (hk_tx, hk_rx) = channel();
    let (hkf_tx, hkf_rx) = channel();
    let (q_tx, q_rx) = channel();

    let res = runner::run(move |proxy| {
        let wake = move |p: &winit::event_loop::EventLoopProxy<runner::UserEvent>| {
            let p = p.clone();
            move || {
                let _ = p.send_event(runner::UserEvent::Wake);
            }
        };
        ui::spawn_worker(&paths, &shared, tx, wake(&proxy));
        platform::hotkeys(hk_tx, hkf_tx, wake(&proxy));
        let w = wake(&proxy);
        platform::watch_quit(move || {
            let _ = q_tx.send(());
            w();
        });
        ui::Overlay::new(t, paths, shared, rx, hk_rx, hkf_rx, q_rx)
    });
    if let Err(e) = res {
        let p = files::Paths::new();
        files::log_error(&p.err, &format!("啟動失敗：{e}"));
        platform::message_box(&format!("經驗收益計算器啟動失敗，詳細內容在 error.log。\n\n{e}"), 0x10);
    }
}
