use std::thread;
use log::{debug, error, info};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::OnceCell;
use std::{path::Path, sync::mpsc};
use std::sync::Mutex;

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

        Config::get_index_dir_paths().unwrap().iter().for_each(|path| {
            watcher.watch(Path::new(path), RecursiveMode::Recursive).unwrap();
        });

        thread::Builder::new()
            .name("file-monitor".into())
            .spawn(move || {
                let worker = Worker::new().unwrap();
                for res in rx {
                    match res {
                        Ok(event) => {
                            match event.kind {
                                notify::EventKind::Create(_) | notify::EventKind::Modify(_) | notify::EventKind::Remove(_) => {
                                    for path in &event.paths {
                                        debug!("文件被变更: {:?}, {:?}", event.kind, path);
                                        worker.submit_index_all_files(path).unwrap();
                                    }
                                },
                                notify::EventKind::Access(_) => {
                                    // 访问事件不需要重新索引
                                },
                                notify::EventKind::Other => {
                                    debug!("其他文件系统事件: {:?}", event.paths);
                                },
                                _ => {
                                    debug!("未知的事件类型: {:?}", event);
                                }
                            }
                        },
                        Err(e) => error!("监听错误: {:?}", e),
                    }
                }
            }).unwrap();

        Mutex::new(Monitor{watcher})
    })
}

pub fn add_watched_path(new_path: &str) {
    info!("设置新的监听路径: {:?}", new_path);
    let mut monitor = get_monitor().lock().unwrap();

    // 添加新的监听路径
    match monitor.watcher.watch(Path::new(new_path), RecursiveMode::Recursive) {
        Ok(_) => {
            info!("成功添加新的监听路径: {}", new_path);
        },
        Err(e) => {
            error!("添加新的监听路径失败: {}, 错误: {:?}", new_path, e);
        }
    }
}

pub fn del_watched_path(old_path: &str) {
    info!("删除旧的监听路径: {:?}", old_path);
    let mut monitor = get_monitor().lock().unwrap();

    match monitor.watcher.unwatch(Path::new(old_path)) {
        Ok(_) => {
            info!("成功移除旧的监听路径: {}", old_path);
        },
        Err(e) => {
            error!("移除旧的监听路径失败: {}, 错误: {:?}", old_path, e);
        }
    }
}
