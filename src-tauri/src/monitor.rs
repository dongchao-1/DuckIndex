use std::thread;
use log::{debug, error};
use notify::{Event, RecursiveMode, Result, Watcher};
use std::{path::Path, sync::mpsc};

use crate::config::Config;
use crate::Worker;

pub struct Monitor {
}

impl Monitor {
    pub fn start_monitor() {
        thread::Builder::new()
            .name("file-monitor".into())
            .spawn(|| loop {
                if let Err(e) = Monitor::watch() {
                    error!("watch error: {:?}", e);
                }
            }).unwrap();
    }

    fn watch() -> Result<()> {
        let (tx, rx) = mpsc::channel::<Result<Event>>();

        let mut watcher = notify::recommended_watcher(tx)?;

        Config::get_index_dir_paths().unwrap().iter().for_each(|path| {
            watcher.watch(Path::new(path), RecursiveMode::Recursive).unwrap();
        });

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
        Ok(())
    }
}