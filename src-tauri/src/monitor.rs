use std::thread;
use log::{debug, error, info};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::OnceCell;
use std::{path::Path, sync::mpsc};
use std::collections::HashSet;
use std::sync::Mutex;

use crate::config::Config;
use crate::Worker;

pub struct Monitor {
    watcher: RecommendedWatcher,
    watched_paths: HashSet<String>,
}

static WATCHER: OnceCell<Mutex<Monitor>> = OnceCell::new();

pub fn get_monitor() -> &'static Mutex<Monitor> {
    WATCHER.get_or_init(|| {
        info!("初始化 WATCHER");
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx).unwrap();
        let mut watched_paths = HashSet::new();

        Config::get_index_dir_paths().unwrap().iter().for_each(|path| {
            watcher.watch(Path::new(path), RecursiveMode::Recursive).unwrap();
            watched_paths.insert(path.to_string());
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

        Mutex::new(Monitor{watcher, watched_paths})
    })
}

pub fn set_watched_paths(new_paths: Vec<String>) {
    info!("设置新的监听路径: {:?}", new_paths);
    let monitor_mutex = get_monitor().lock().unwrap();
    let mut monitor = monitor_mutex;

    let old_paths = HashSet::from(monitor.watched_paths.clone());
    let new_paths = HashSet::from_iter(new_paths);

    // 移除旧的监听路径
    for path in old_paths.difference(&new_paths) {
        match monitor.watcher.unwatch(Path::new(path)) {
            Ok(_) => {
                monitor.watched_paths.remove(path);
                info!("成功移除旧的监听路径: {}", path);
            },
            Err(e) => {
                error!("移除旧的监听路径失败: {}, 错误: {:?}", path, e);
            }
        }
    }
    // 添加新的监听路径
    for path in new_paths.difference(&old_paths) {
        match monitor.watcher.watch(Path::new(path), RecursiveMode::Recursive) {
            Ok(_) => {
                monitor.watched_paths.insert(path.to_string());
                info!("成功添加新的监听路径: {}", path);
            },
            Err(e) => {
                error!("添加新的监听路径失败: {}, 错误: {:?}", path, e);
            }
        }
    }

    info!("监听路径重新设置完成，当前监听 {} 个路径", &monitor.watched_paths.len());
}
