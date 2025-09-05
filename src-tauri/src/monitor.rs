use anyhow::Result;
use log::{debug, error, info};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::OnceCell;
use std::sync::Mutex;
use std::thread;
use std::{path::Path, sync::mpsc};

use crate::config::Config;
use crate::Worker;

pub struct Monitor {
    watcher: RecommendedWatcher,
}

static MONITOR: OnceCell<Mutex<Monitor>> = OnceCell::new();

pub fn get_monitor() -> &'static Mutex<Monitor> {
    MONITOR.get_or_init(|| {
        info!("初始化 WATCHER");
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx).unwrap();

        Config::get_index_dir_paths()
            .unwrap()
            .iter()
            .for_each(|path| {
                watcher
                    .watch(Path::new(path), RecursiveMode::Recursive)
                    .unwrap();
            });

        thread::Builder::new()
            .name("file-monitor".into())
            .spawn(move || {
                let worker = Worker::new().unwrap();
                for res in rx {
                    match res {
                        Ok(event) => {
                            match event.kind {
                                notify::EventKind::Create(_)
                                | notify::EventKind::Modify(_)
                                | notify::EventKind::Remove(_) => {
                                    for path in &event.paths {
                                        debug!("文件被变更: {:?}, {}", event.kind, path.display());
                                        if let Err(e) = worker.submit_index_all_files(path) {
                                            error!(
                                                "提交索引任务失败: {}, 错误: {:?}",
                                                path.display(),
                                                e
                                            );
                                        }
                                    }
                                }
                                notify::EventKind::Access(_) => {
                                    // 访问事件不需要重新索引
                                }
                                notify::EventKind::Other => {
                                    debug!("其他文件系统事件: {:?}", event.paths);
                                }
                                _ => {
                                    debug!("未知的事件类型: {event:?}");
                                }
                            }
                        }
                        Err(e) => error!("监听错误: {e:?}"),
                    }
                }
            })
            .unwrap();

        Mutex::new(Monitor { watcher })
    })
}

pub fn add_watched_path(new_path: &Path) -> Result<()> {
    info!("设置新的监听路径: {}", new_path.display());
    let mut monitor = get_monitor()
        .lock()
        .map_err(|e| anyhow::anyhow!("Failed to acquire monitor lock: {}", e))?;

    monitor.watcher.watch(new_path, RecursiveMode::Recursive)?;
    Ok(())
}

pub fn del_watched_path(old_path: &Path) -> Result<()> {
    info!("删除旧的监听路径: {}", old_path.display());
    let mut monitor = get_monitor()
        .lock()
        .map_err(|e| anyhow::anyhow!("Failed to acquire monitor lock: {}", e))?;

    monitor.watcher.unwatch(old_path)?;
    Ok(())
}
