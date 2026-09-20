//! `mdl watch`（Task 4.4）：文件系统监听 → 防抖 → 增量重建 → 运行/校验。
//! 可测核心：防抖调度（时间注入）与 重建+运行 组合；notify 线程是薄胶水。

use std::path::Path;
use std::time::{Duration, Instant};

use metado_engine::KeyPair;

use crate::{build_plugin, verify_mdl, BuildInfo};

#[cfg(feature = "watch")]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

/// 防抖调度器：连续变更在窗口内收敛为一次 reload。
pub struct WatchScheduler {
    debounce: Duration,
    pending: bool,
    since: Instant,
}

impl WatchScheduler {
    pub fn new(debounce: Duration) -> Self {
        Self {
            debounce,
            pending: false,
            since: Instant::now(),
        }
    }

    /// 收到一次变更；返回 true 表示此刻应触发 reload（防抖窗口已过且此前有 pending 变更）。
    pub fn on_change(&mut self) -> bool {
        if self.debounce.is_zero() {
            return true;
        }
        if !self.pending {
            self.pending = true;
            self.since = Instant::now();
            return false;
        }
        if self.pending && self.since.elapsed() >= self.debounce {
            self.pending = false;
            return true;
        }
        false
    }

    /// 触发成功（重建完成）后复位。
    pub fn record_reload(&mut self) {
        self.pending = false;
        self.since = Instant::now();
    }
}

/// 重建 + 校验（watch 每次 reload 的增量为"目录→签名 zip→校验"整条链）。
pub fn build_and_run(dir: &Path, kp: &KeyPair) -> Result<BuildInfo, String> {
    let bytes = build_plugin(dir, kp)?;
    verify_mdl(&bytes)
}

#[cfg(feature = "watch")]
pub fn run_watch(
    dir: &Path,
    key: &Path,
    output: &Path,
    debounce_ms: u64,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    use notify::{Config, Event, RecursiveMode, RecommendedWatcher, Watcher};

    let kp = crate::KeyStore::load(key)
        .map_err(|_| format!("load key {}: file missing (run 'mdl build' once to generate)", key.display()))?;

    let debounce = Duration::from_millis(debounce_ms);
    let scheduler = WatchScheduler::new(debounce);

    // 首个构建
    match build_and_run(dir, &kp) {
        Ok(info) => {
            std::fs::write(output, build_plugin(dir, &kp)?)
                .map_err(|e| format!("write {}: {}", output.display(), e))?;
            println!("[watch] built {} ({})", output.display(), info.plugin_name);
        }
        Err(msg) => eprintln!("[watch] initial build failed: {}", msg),
    }

    let shared = Arc::new(Mutex::new(scheduler));
    let shared_cl = shared.clone();
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if res.is_ok() {
                if let Ok(mut sch) = shared_cl.lock() {
                    if sch.on_change() {
                        eprintln!("[watch] change detected -> rebuilding ({}ms)", debounce_ms);
                    }
                }
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(200)),
    )
    .map_err(|e| format!("watcher init: {}", e))?;

    watcher
        .watch(dir, RecursiveMode::Recursive)
        .map_err(|e| format!("watch {}: {}", dir.display(), e))?;

    println!("[watch] watching {} (press Ctrl-C to stop)", dir.display());
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(250));
        let fire = shared
            .lock()
            .map(|mut s| {
                let fire = s.on_change();
                if fire {
                    match build_and_run(dir, &kp) {
                        Ok(info) => {
                            if let Ok(bytes) = build_plugin(dir, &kp) {
                                if std::fs::write(output, &bytes).is_ok() {
                                    println!("[watch] rebuilt {} ({})", info.plugin_name, output.display());
                                }
                            }
                            s.record_reload();
                        }
                        Err(msg) => eprintln!("[watch] rebuild failed: {}", msg),
                    }
                }
                fire
            })
            .unwrap_or(false);
        if fire {
            eprintln!("[watch] done");
        }
    }
    Ok(())
}